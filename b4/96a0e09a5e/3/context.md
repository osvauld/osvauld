# Session Context

## User Prompts

### Prompt 1

Implement the following plan:

# Observability Gaps Implementation Plan

## Context

After debugging a "viewer can't write to dynamic channels" issue using the JSONL capture system, we identified 8 categories of observability gaps. P0 items are already implemented and verified. This plan covers the remaining P1-P3 items.

## Already Done (P0) — Verified

### permission_check (`scribe/src/permit.rs` + `scribe/src/state.rs`)
- `emit_permission_check_capture()` in `state.rs:312-337` — fields: t...

