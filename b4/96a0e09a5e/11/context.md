# Session Context

## User Prompts

### Prompt 1

Implement the following plan:

# Plan: Isolate App Panics from Sthalam Shell

## Context

When a Lua app sends malformed data to the Slint interpreter (e.g., wrong field names for a struct), the interpreter panics on the main thread. Since all Slint app windows share the main event loop via timer callbacks, this panic kills the entire process — crashing the shell and all other running apps. The goal is to catch these panics per-app so only the offending app dies.

## Approach: `catch_unwind` a...

