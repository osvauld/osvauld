# Observability: Event Capture System

## Overview

The event capture system records structured events from Courier and Scribe into JSONL files for debugging and AI analysis. It captures:

- **CourierEvents**: PeerAuthenticated, SpacePublished, ViewerSyncComplete, etc.
- **MessageTraces**: Protocol messages (Hello, Welcome, SyncOffer, etc.) with direction + node IDs
- **PageUpdates**: Layer changes, ephemeral data, peer subscriptions
- **SyncEvents**: EnsureSync, NewDynamicLayer, LayerAccessChanged
- **LayerAuth**: DID authorization/pending events per layer (who got access, when, why)
- **Tracing logs** (optional): All `tracing` log events from all crates

Capture is supported on both **kunki** (node) and **sthalam** (desktop shell) instances.

## JSONL Format

Each line is a self-contained JSON object with a `type` field and `ts` timestamp:

```json
{"type": "courier_event", "ts": "2026-02-11T15:30:00.123Z", "data": {"PeerAuthenticated": {"node_id": "abc...", "did": "did:key:...", "username": "alice"}}, "instance": "node"}
{"type": "message_trace", "ts": "2026-02-11T15:30:00.234Z", "direction": "Sent", "msg": "Hello", "node_id": "abc...", "peer_node_id": "def...", "instance": "owner"}
{"type": "page_update", "ts": "2026-02-11T15:30:00.345Z", "page_id": "pg_123", "data": {"LayerChanged": {"layer": "messages", ...}}, "instance": "node"}
{"type": "sync_event", "ts": "2026-02-11T15:30:00.456Z", "page_id": "pg_123", "data": "EnsureSync { user_did: \"did:key:...\" }", "instance": "node"}
{"type": "layer_auth", "ts": 1739284200567, "page_id": "pg_123", "layer": "messages/project-x", "did": "did:key:...", "action": "authorized", "instance": "node"}
{"type": "log", "ts": "2026-02-11T15:30:00.567Z", "level": "INFO", "target": "courier::coordinator", "msg": "Peer authenticated", "instance": "owner"}
```

## Usage from Python

### Single instance

```python
from osvauld.client import ControlClient

client = ControlClient("/tmp/test/node.sock")

# Start capturing (structured events only)
client.capture_start("/tmp/captures/node.jsonl")

# ... run operations ...

# Stop and flush
client.capture_end()
```

### With tracing logs

```python
client.capture_start("/tmp/captures/node.jsonl", include_logs=True)
```

### Multi-peer via AppTestScenario (E2E tests)

```python
from osvauld.scenario import AppTestScenario

DEMOS_APP = "sample_apps/osvauld-demos"

with AppTestScenario(
    name="my_test",
    app_path=DEMOS_APP,
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
    },
) as s:
    # Start capture on ALL instances (node + all peers)
    s.capture_start("my_label")

    # Run operations
    alice = s.peer("alice")
    alice.eval('create_channel("project-x")')
    alice.eval('send_message("Hello!")')
    time.sleep(2)

    # Stop and merge into sorted file
    merged = s.capture_end("my_label", merge=True)
    # → /tmp/my_test/captures/my_label_merged.jsonl
```

Output files:
```
/tmp/my_test/captures/
├── my_label_node.jsonl       # node events
├── my_label_alice.jsonl      # owner events
├── my_label_bob.jsonl        # viewer events
└── my_label_merged.jsonl     # all events sorted by timestamp
```

### Multi-peer via Scenario (low-level)

```python
from osvauld.scenario import Scenario

with Scenario(owner=1, node=1, viewer=1) as s:
    s.setup_owner(app_path="sample_apps/my-shop")
    s.connect_owner_to_node()

    s.capture_start("test_sync")

    # Run operations ...

    merged = s.capture_end("test_sync", merge=True)
    # → /tmp/osvauld_test/captures/test_sync_merged.jsonl
```

## E2E Test Scripts

| Script | What it tests |
|--------|--------------|
| `e2e_tests/test_chat.py` | Bidirectional message sync (3 users), captures events |
| `e2e_tests/test_custom_channel.py` | Dynamic channel creation + cross-peer sync, captures events |

```bash
# Run with capture
python e2e_tests/test_chat.py

# Also capture tracing logs (verbose)
python e2e_tests/test_custom_channel.py --capture-logs

# Keep tmux alive after test for debugging
python e2e_tests/test_custom_channel.py --keep
```

## Filtering Captures

```bash
# All sync events (EnsureSync, NewDynamicLayer, LayerAccessChanged)
grep '"type": "sync_event"' merged.jsonl

# Protocol messages (Hello, Welcome, SyncOffer, etc.)
grep '"type": "message_trace"' merged.jsonl

# Layer changes only
grep 'LayerChanged' merged.jsonl

# Dynamic layer creation
grep 'NewDynamicLayer' merged.jsonl

# Layer authorization events (who got access to which layer)
grep '"type": "layer_auth"' merged.jsonl

# Pending authorizations (auth arrived before layer existed)
grep '"action": "pending"' merged.jsonl

# Events from a specific instance
grep '"instance": "alice"' merged.jsonl

# Pretty-print
cat merged.jsonl | python -m json.tool
```

## Architecture

```
Courier (PeerActor)                    Scribe (Actor)
  │ CourierEvent                         │ PageUpdate
  │ MessageTrace                         │ SyncEvent
  │                                      │
  └─→ serialize to JSON ──┐   ┌── serialize to JSON ←─┘
                           │   │
                    broadcast::Sender<String>
                           │   │
                    ┌──────┴───┴──────┐
                    │  CaptureHandle   │
                    │  (logging_utils) │
                    │                  │
                    │  Subscribes to   │
                    │  all registered  │
                    │  sources         │
                    └────────┬─────────┘
                             │
                     tokio::fs::File
                             │
                      output.jsonl
```

Each crate serializes its own events into pre-serialized JSON strings, keeping `logging_utils` free from courier/scribe type dependencies. The `CaptureHandle` just collects `String` lines from `broadcast::Sender<String>` channels.

### Wiring

| Binary | How capture is wired |
|--------|---------------------|
| **kunki** | `init_rich_tracing_with_capture()` → `CaptureHandle` → registered with Courier + Butler → control server `capture_start`/`capture_end` |
| **sthalam** | `init_rich_tracing_with_capture()` → `CaptureHandle` → broadcast channel registered + set on Butler → passed to `init_p2p()` → Courier gets `capture_tx` → control server `capture_start`/`capture_end` |

## Build

No extra build flags required. The broadcast channels are always compiled in. When no capture is active, `broadcast::send()` with 0 receivers is a no-op (near-zero cost).

## AI Analysis

Feed the merged JSONL file to an AI for debugging sync issues:

```
Here's a capture of a chat sync session between alice (owner),
node, and bob (viewer). Bob can't send messages in a custom channel.
Analyze the event flow and identify where it breaks down.

<attach my_label_merged.jsonl>
```

The AI can trace the full event flow: owner creates channel → node detects dynamic layer → node issues permits → broadcasts to peers → peers receive layer access → messages sync (or don't).
