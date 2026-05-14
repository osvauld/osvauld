//! Synth — drives Raylib AudioStream directly from music_ir.
//!
//! No WAV files, no disk. music_ir → PCM f32 samples → AudioStream each frame.
//!
//! # Design
//!
//! A `Synth` holds a queue of `Note`s parsed from `music_ir`. Each frame
//! the game loop calls `Synth::fill_stream()` which generates the next chunk
//! of PCM samples and hands them to Raylib's `UpdateAudioStream`.
//!
//! Voices are simple additive oscillators (sine for now, timbre selectable).
//! The `music_ir` timing grid (bpm + ticks_per_beat) maps ticks → sample
//! offsets. Velocity → amplitude. `gamaka` / `andolan` modulations come from
//! the resonance_ir energy field that drives a per-note LFO.
//!
//! # Timbre
//!
//! | music_ir timbre | waveform        |
//! |-----------------|-----------------|
//! | "piano"         | sine + decay    |
//! | "sawtooth"      | sawtooth        |
//! | "triangle"      | triangle        |
//! | "square"        | square          |
//! | "sine"          | pure sine       |
//! | "metal"         | noisy sine      |
//! | anything else   | sine            |

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde::Deserialize;

// ── Constants ────────────────────────────────────────────────────────────────

pub const SAMPLE_RATE: u32 = 44_100;
pub const CHANNELS: u32 = 1; // mono; stereo extension is pan in mix stage
pub const SAMPLE_SIZE: u32 = 32; // f32
/// Frames handed to AudioStream per update call. Keep small for low latency.
pub const STREAM_BUFFER_FRAMES: usize = 512;

// ── Pitch table ──────────────────────────────────────────────────────────────

/// Convert a pitch string like "g4", "c3", "a4" to Hz.
/// Follows scientific pitch notation: a4 = 440 Hz.
pub fn pitch_to_hz(pitch: &str) -> f32 {
    let pitch = pitch.to_lowercase();
    let bytes = pitch.as_bytes();
    if bytes.is_empty() {
        return 440.0;
    }
    let note_char = bytes[0] as char;
    let (accidental, octave_str) = if bytes.len() > 1 && (bytes[1] == b'#' || bytes[1] == b'b') {
        (bytes[1] as char, &pitch[2..])
    } else {
        (' ', &pitch[1..])
    };
    let octave: i32 = octave_str.parse().unwrap_or(4);

    // Semitone offset from C
    let semitone_from_c: i32 = match note_char {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => 9, // fallback to A
    };
    let accidental_offset: i32 = match accidental {
        '#' => 1,
        'b' => -1,
        _ => 0,
    };
    let semitone = semitone_from_c + accidental_offset;
    // MIDI: C4 = 60, A4 = 69; Hz = 440 * 2^((midi - 69) / 12)
    let midi = (octave + 1) * 12 + semitone;
    440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
}

// ── Waveform ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Waveform {
    Sine,
    Sawtooth,
    Triangle,
    Square,
    PianoDecay, // sine with exponential decay envelope
    Metal,      // sine + slight frequency noise
}

impl Waveform {
    pub fn from_timbre(timbre: &str) -> Self {
        match timbre {
            "piano" => Waveform::PianoDecay,
            "sawtooth" => Waveform::Sawtooth,
            "triangle" => Waveform::Triangle,
            "square" => Waveform::Square,
            "metal" => Waveform::Metal,
            _ => Waveform::Sine,
        }
    }

    pub fn sample(self, phase: f32, elapsed_fraction: f32, noise: f32) -> f32 {
        let tau = std::f32::consts::TAU;
        match self {
            Waveform::Sine => (phase * tau).sin(),
            Waveform::Sawtooth => 2.0 * (phase - phase.floor()) - 1.0,
            Waveform::Triangle => {
                let p = phase - phase.floor();
                if p < 0.5 {
                    4.0 * p - 1.0
                } else {
                    3.0 - 4.0 * p
                }
            }
            Waveform::Square => {
                if (phase - phase.floor()) < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::PianoDecay => {
                let env = (-5.0 * elapsed_fraction).exp();
                (phase * tau).sin() * env
            }
            Waveform::Metal => {
                // sine with slight pitch wobble from noise
                ((phase + noise * 0.01) * tau).sin()
            }
        }
    }
}

// ── Note ─────────────────────────────────────────────────────────────────────

/// One scheduled note in the synthesizer queue.
#[derive(Debug, Clone)]
pub struct Note {
    pub hz: f32,
    pub amplitude: f32, // 0.0–1.0 (from velocity)
    pub pan: f32,       // -1.0 (left) to 1.0 (right); 0 = center
    pub waveform: Waveform,
    pub lfo_rate: f32,  // Hz — modulation from resonance_ir energy
    pub lfo_depth: f32, // 0.0–1.0
    pub duration_samples: u64,
    /// Sample index at which this note starts (relative to stream position)
    pub start_sample: u64,
}

// ── Active voice ──────────────────────────────────────────────────────────────

struct Voice {
    note: Note,
    phase: f32,
    lfo_phase: f32,
    samples_rendered: u64,
    noise_seed: u32,
}

impl Voice {
    fn new(note: Note) -> Self {
        Self {
            note,
            phase: 0.0,
            lfo_phase: 0.0,
            samples_rendered: 0,
            noise_seed: 12345,
        }
    }

    /// Returns true while voice is still active.
    fn render_into(&mut self, buf: &mut [f32]) -> bool {
        let sr = SAMPLE_RATE as f32;
        for s in buf.iter_mut() {
            if self.samples_rendered >= self.note.duration_samples {
                return false;
            }
            // LFO modulates amplitude (tremolo)
            let lfo = (self.lfo_phase * std::f32::consts::TAU).sin();
            let amp = self.note.amplitude * (1.0 - self.note.lfo_depth * 0.5 * (1.0 - lfo));

            // Cheap LCG noise for Metal waveform
            self.noise_seed = self
                .noise_seed
                .wrapping_mul(1664525)
                .wrapping_add(1013904223);
            let noise = (self.noise_seed as f32 / u32::MAX as f32) * 2.0 - 1.0;

            let elapsed_fraction = self.samples_rendered as f32 / self.note.duration_samples as f32;
            let sample = self
                .note
                .waveform
                .sample(self.phase, elapsed_fraction, noise)
                * amp;

            *s += sample;

            self.phase += self.note.hz / sr;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }

            self.lfo_phase += self.note.lfo_rate / sr;
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= 1.0;
            }

            self.samples_rendered += 1;
        }
        true
    }
}

// ── music_ir deserialization (minimal) ───────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MusicIr {
    pub timing: IrTiming,
    pub voices: Vec<IrVoice>,
    pub events: Vec<IrEvent>,
}

#[derive(Debug, Deserialize)]
pub struct IrTiming {
    pub bpm: u32,
    pub ticks_per_beat: u32,
}

#[derive(Debug, Deserialize)]
pub struct IrVoice {
    pub voice_id: String,
    pub timbre: String,
    pub gain: f32,
    pub pan: f32,
}

#[derive(Debug, Deserialize)]
pub struct IrEvent {
    pub t_start_tick: u64,
    pub t_dur_tick: u64,
    pub voice_id: String,
    pub pitch: String,
    pub velocity: f32,
}

impl MusicIr {
    /// Convert music_ir timing + events into a flat list of scheduled Notes.
    ///
    /// tick → sample = tick * (60 / bpm / ticks_per_beat) * SAMPLE_RATE
    pub fn to_notes(&self, resonance_energy: f32) -> Vec<Note> {
        let bpm = self.bpm() as f32;
        let tpb = self.ticks_per_beat() as f32;
        let secs_per_tick = 60.0 / bpm / tpb;
        let samples_per_tick = secs_per_tick * SAMPLE_RATE as f32;

        let voice_map: std::collections::HashMap<&str, &IrVoice> = self
            .voices
            .iter()
            .map(|v| (v.voice_id.as_str(), v))
            .collect();

        self.events
            .iter()
            .filter_map(|ev| {
                let voice = voice_map.get(ev.voice_id.as_str())?;
                let hz = pitch_to_hz(&ev.pitch);
                let amplitude = (ev.velocity * voice.gain).clamp(0.0, 1.0);
                let waveform = Waveform::from_timbre(&voice.timbre);
                let duration_samples = ((ev.t_dur_tick as f32) * samples_per_tick) as u64;
                let start_sample = ((ev.t_start_tick as f32) * samples_per_tick) as u64;
                // resonance energy drives LFO depth — more active graph = more movement
                let lfo_depth = (resonance_energy * 0.3).clamp(0.0, 0.5);
                Some(Note {
                    hz,
                    amplitude,
                    pan: voice.pan,
                    waveform,
                    lfo_rate: 5.0 + resonance_energy * 3.0, // 5–8 Hz tremolo
                    lfo_depth,
                    duration_samples,
                    start_sample,
                })
            })
            .collect()
    }

    fn bpm(&self) -> u32 {
        self.timing.bpm.max(1)
    }
    fn ticks_per_beat(&self) -> u32 {
        self.timing.ticks_per_beat.max(1)
    }
}

// ── Synth ─────────────────────────────────────────────────────────────────────

/// Shared synth state — clone the Arc to pass between game loop and Lua bindings.
pub type SharedSynth = Arc<Mutex<Synth>>;

pub struct Synth {
    /// Notes waiting to become voices (sorted by start_sample)
    pending: VecDeque<Note>,
    /// Currently rendering voices
    active: Vec<Voice>,
    /// Absolute sample counter (advances every fill_stream call)
    sample_pos: u64,
    /// Mix buffer reused across calls
    mix_buf: Vec<f32>,
}

impl Synth {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            active: Vec::new(),
            sample_pos: 0,
            mix_buf: vec![0.0f32; STREAM_BUFFER_FRAMES],
        }
    }

    /// Load notes from a music_ir JSON string, replacing any pending notes.
    /// Active voices finish naturally.
    ///
    /// `resonance_energy` comes from resonance_ir (e.g. top node energy).
    pub fn load_music_ir(&mut self, json: &str, resonance_energy: f32) {
        match serde_json::from_str::<MusicIr>(json) {
            Ok(ir) => {
                let mut notes = ir.to_notes(resonance_energy);
                // Offset start_samples to current position so playback starts now
                let offset = self.sample_pos;
                for n in &mut notes {
                    n.start_sample += offset;
                }
                notes.sort_by_key(|n| n.start_sample);
                self.pending = notes.into();
                tracing::debug!(pending = self.pending.len(), "Synth loaded music_ir");
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse music_ir JSON");
            }
        }
    }

    /// Generate the next `STREAM_BUFFER_FRAMES` PCM samples.
    /// Call this every frame before `UpdateAudioStream`.
    ///
    /// Returns a slice of f32 samples in [-1.0, 1.0].
    pub fn fill(&mut self) -> &[f32] {
        let buf = &mut self.mix_buf[..STREAM_BUFFER_FRAMES];
        for s in buf.iter_mut() {
            *s = 0.0;
        }

        // Activate pending notes whose start_sample has arrived.
        while let Some(note) = self.pending.front() {
            if note.start_sample <= self.sample_pos {
                let note = self.pending.pop_front().unwrap();
                self.active.push(Voice::new(note));
            } else {
                break;
            }
        }

        self.active.retain_mut(|voice| voice.render_into(buf));

        // Soft clip to [-1, 1] — divide by sqrt(active_count) to prevent saturation.
        let n_active = self.active.len() as f32;
        let norm = if n_active > 1.0 {
            1.0 / n_active.sqrt()
        } else {
            1.0
        };
        for s in buf.iter_mut() {
            *s = (*s * norm).clamp(-1.0, 1.0);
        }

        self.sample_pos += STREAM_BUFFER_FRAMES as u64;
        buf
    }

    pub fn is_playing(&self) -> bool {
        !self.active.is_empty() || !self.pending.is_empty()
    }

    /// Stop all playback immediately.
    pub fn stop(&mut self) {
        self.active.clear();
        self.pending.clear();
    }
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}
