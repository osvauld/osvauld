# Session Context

## User Prompts

### Prompt 1

Implement the following plan:

# JSON Logging & Event Capture System

## Context

We have event systems spread across Courier and Scribe, plus a dormant log capture layer:

| System | Location | Status | Captures |
|--------|----------|--------|----------|
| `CourierEvent` | `courier/src/handle.rs:26` | **Active** | App events: PeerAuthenticated, SpacePublished, ViewerSyncComplete |
| `MessageTrace` | `courier/src/trace.rs` | **Test-only** (None in prod) | Protocol messages: Hello, Welcome with ...

