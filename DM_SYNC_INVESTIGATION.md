# DM Sync Delay Investigation

## Problem
DM messages only converge after sending multiple messages, but once converged, sync is fast.

## Root Cause Analysis

### Issue 1: Static Layers Created via EnsureLoroMap/List Don't Write to __sync_meta

**Symptom**: Messages on new channels don't reach viewers.

**Root Cause**: Two code paths for creating layers behave differently:

| Path | Function | Writes to __sync_meta? |
|------|----------|------------------------|
| `scribe:create_layer()` | `handle_create_dynamic_layer` | ✅ Yes (lines 99-108) |
| `scribe:list()` / `scribe:map()` | `ensure_layer_for_lua` | ❌ No |

**The Flow When Using EnsureLoroMap/List**:

1. **Owner creates layer** via `EnsureLoroList/Map` (`scribe:list("channels/general/messages")`)
2. `ensure_layer_for_lua` creates the layer but does NOT write to `__sync_meta`
3. `auto_subscribe_sync_target` only subscribes the node (sync_target)
4. Loro observer broadcasts to subscribers (only the node)

5. **Node receives SyncOffer**:
   - `create_layer_from_peer` creates the layer
   - `find_matching_dynamic_schema` finds `channels/{id}/messages` matches
   - `is_dynamic = true`, so subscribers are NOT added
   - Code comment: "Dynamic layers are access-controlled via __sync_meta"

6. **Result**: Node has no subscribers to broadcast to, viewers never receive updates

**Key Files**:

| File | Line | Issue |
|------|------|-------|
| `scribe/src/actor.rs` | 840-853 | `ensure_layer_for_lua` doesn't write to `__sync_meta` |
| `scribe/src/layer_unit/dynamic.rs` | 99-108 | `handle_create_dynamic_layer` DOES write to `__sync_meta` |
| `scribe/src/sync/apply.rs` | 1062-1085 | `create_layer_from_peer` skips subscribers for dynamic layers |

**Fix**: In `ensure_layer_for_lua`, check if the layer matches a dynamic schema and write to `__sync_meta`:

```rust
fn ensure_layer_for_lua(state: &mut ScribeState, layer_name: &str, kind: &str) {
    // ... existing code ...
    
    // If layer matches a dynamic schema, write to __sync_meta for discovery
    if let Some(permit) = &state.our_permit {
        if let Some(_) = crate::layer_unit::find_matching_dynamic_schema(permit, layer_name, &state.page_id) {
            let our_did = state.our_did.clone();
            let meta_layer = sync_meta::sync_meta_layer_name(&our_did);
            if state.units.contains_key(&meta_layer) {
                sync_meta::write_sync_meta_entry(state, &our_did, &layer_name, false);
                info!(layer = %layer_name, "Wrote __sync_meta entry for ensure_layer");
            }
        }
    }
}
```

### Issue 2: Viewer Not Subscribed to __sync_meta for Fan-out

**The Flow**:
1. **Owner creates DM** → writes to owner's `__sync_meta:owner_did`
2. **Owner grants access** (`AddLayerAccess`) → re-writes `__sync_meta:owner_did` with `synced=false`
3. **Node stores authority** → fans out to viewer's `__sync_meta:viewer_did`
4. **Viewer should detect** → check unsynced entries, send LayerSubscribe

### The Gap
The viewer only detects `__sync_meta` updates in ONE place:

**`courier/src/peer_actor/sync/protocol.rs:500-502`:**
```rust
if layer_name.starts_with("__sync_meta:") && state.mode == CourierMode::User {
    self.check_sync_meta_and_subscribe(page_id, state).await;
}
```

This happens AFTER the viewer applies a SyncOffer/SyncAccept cycle.

### The Problem
When node fans out to viewer's `__sync_meta:viewer_did`:
1. Node writes entry via `write_sync_meta_entry()` in `scribe/src/sync/sync_meta.rs`
2. Loro observer triggers broadcast to layer subscribers
3. **BUT**: Viewer must already be subscribed to their `__sync_meta:viewer_did` layer

The issue is likely one of:
1. **Timing**: Viewer isn't subscribed to their `__sync_meta` when fanout happens
2. **Missing subscription**: Viewer never subscribed to their own `__sync_meta`
3. **Race condition**: Messages sent before subscription is established

## Key Files

| File | Purpose |
|------|---------|
| `scribe/src/sync/sync_meta.rs:479-516` | `handle_fan_out_layer_to_users` |
| `scribe/src/layer_unit/dynamic.rs:226-326` | `handle_add_layer_access` |
| `courier/src/peer_actor/sync/protocol.rs:497-502` | `__sync_meta` detection trigger |
| `courier/src/peer_actor/subscribe.rs:160-200` | `check_sync_meta_and_subscribe` |

## TODO

1. **Check when viewer subscribes to their `__sync_meta`**
   - Look at initial handshake flow
   - Verify `__sync_meta:viewer_did` is in static layers
   - Check if subscription happens before fanout

2. **Check broadcast timing**
   - Does `write_sync_meta_entry` trigger observer immediately?
   - Is the viewer's subscriber entry on the layer correct?

3. **Verify fanout path**
   - `handle_add_layer_access` re-writes `__sync_meta` with `synced=false`
   - This should trigger `SyncEvent::SubscribeLayers` to node
   - Node should send `LayerSubscribe` to owner
   - Owner responds with authority
   - Node stores authority and fans out

4. **Possible fixes**
   - Ensure viewer is subscribed to their `__sync_meta` during initial handshake
   - Add explicit sync trigger when `AddLayerAccess` is called
   - Consider push-based notification instead of relying on broadcast

## Next Steps

1. Run e2e test with capture logs to trace exact sequence
2. Check if `__sync_meta:viewer_did` subscription is established before DM creation
3. Verify the Loro observer fires correctly for `__sync_meta` layers
4. Check if `ensure_sender_subscribed` in apply.rs handles `__sync_meta` correctly
