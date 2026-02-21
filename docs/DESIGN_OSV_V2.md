# OSV v2 Design Document

> **Status**: Design — not yet implemented.
>
> **Breaking**: v2 is a single breaking wave. All changes ship together.

## 1. Motivation

### 1.1 What v0 does well

The current `app.osv` grammar declares roles, layers, access rules, derivations, and UI app entrypoints. The compiler (`osv_decl`) lowers these into `PermitArtifact` (with `PolicyFacts` and `PolicyRules`) and `RuntimeArtifact` — enough to bootstrap a page and enforce basic RBAC.

### 1.2 What v0 cannot express

Everything between "who can touch this layer" and "what happens when they do" is ad-hoc Lua:

- **Schema and field constraints.** Layer shapes are implicit. Lua validation (`page.lua`) manually checks fields; nothing prevents drift between validation, Lua app, and Slint UI.
- **Mutation behavior.** Defaults, auto-populated fields (timestamps, author DID), immutability after insert — all hand-rolled.
- **State transitions.** `ORDER_STATES` tables live in Lua with manual guard logic.
- **Dynamic layers.** Created via `scribe:create_layer` with string path parsing and regex matching in Lua (`channels.lua:ensure_channel_shard`).
- **Broadcast policy.** All changes broadcast to all subscribers. No field filtering, debouncing, or selective emission.
- **Permit delegation.** `delegation_rules` is `Vec::new()` in the lowered output. There is no grammar for issuing sub-permits.

### 1.3 The `serde_json::Value` problem

`serde_json::Value` is the universal glue across `scribe -> lua_runtime -> renderer_slint`, causing:

- **8+ conversion functions** across `domains/src/layer.rs`, `scribe/src/loro_observer.rs`, `lua_runtime/src/bindings/convert.rs`, `renderer_slint/src/value_convert.rs`, plus a duplicate in `integration_tests/src/tests/validation.rs`.
- **Lossy conversions**: binary becomes base64 strings, int/float conflation (`as_i64` vs `as_f64` guessing), empty Lua tables are ambiguous (array or object).
- **3 serialization hops** per CRDT update reaching the UI: `LoroValue -> serde_json::Value -> LuaValue -> serde_json::Value -> SlintValue`.

v2 introduces `Sthithi` and `Parivarta` as the canonical runtime types to replace `serde_json::Value` on the data path.

## 2. Canonical Terminology

Two terms are used throughout the system. They are never suffixed or combined with other words.

| Term | Origin | Meaning |
|------|--------|---------|
| **Sthithi** | സ്ഥിതി (Malayalam) | Current settled state after CRDT merge |
| **Parivarta** | परिवर्त (Sanskrit) | Incoming change / transformation event |

## 3. Design Axioms

1. **One Reality.** `Sthithi` is the single representation of settled state. No parallel JSON/Lua/Slint copies.
2. **Grammar Is Law.** If the `.osv` file does not declare it, it does not exist at runtime. Roles, fields, transitions, broadcasts — all grammar or nothing.
3. **Determinism Over Convenience.** Timestamps come from `ClockSource`, not `os.time()` or `SystemTime::now()`. Random values come from seeded sources. Tests reproduce exactly.
4. **Functional Core, Effectful Edge.** Validation and transforms are pure functions over `Sthithi` and `Parivarta`. Side effects (storage, network, UI) happen at the boundary.
5. **Convergence Is Sacred.** Every grammar construct must be CRDT-safe. State transitions resolve via LWW with peer ordering. No grammar construct may produce divergent state across peers.
6. **Validation Before Interpretation.** Invalid `Parivarta` = hard reject. No partial application, no "best effort" merging of malformed changes.
7. **Generated DX, Not Manual Drift.** Lua type stubs, Slint property contracts, and validation harnesses are compiler outputs. Developers never hand-maintain parallel definitions.
8. **Explicitness Over Magic.** `from clock` means the field comes from the clock. `default "pending"` means the default is "pending". No implicit behaviors.
9. **Versioned Meaning.** `app "X" version "2.0.0"` carries migration semantics. The runtime knows when a schema changed and can apply transforms.
10. **Small Vocabulary, Deep Semantics.** The grammar uses ~20 keywords with the English sentence pattern `subject verb object [preposition modifier]`. No symbolic operators (`==`, `!=`). Constraints read like prose.

## 4. Grammar Overview

### 4.1 Voice and style

The `.osv` grammar reads as English sentences:

```
subject verb object [preposition modifier]
```

Keywords: `as`, `to`, `on`, `where`, `is`, `from`, `using`, `allow`, `for`, `by`, `can`, `default`, `required`, `immutable`, `transitions`, `broadcast`, `issue`, `debounce`, `retain`, `shard`, `create`, `discover`, `validate`, `derive`, `relay`, `order`.

No type annotations (Lua is dynamic — we declare constraints, not types). No symbolic operators.

### 4.2 Top-level structure

```osv
app "my-shop" version "2.0.0" {
  // Roles
  role owner can share, delegate, accept_publish, manage_access;
  role customer;

  // Entity schemas (Sthithi shape)
  sthithi Order {
    field status default "pending";
    field total required;
    field customer_did from peer_did immutable;
    field created_at from clock immutable;
    field updated_at from clock;

    transitions status {
      "pending" to "confirmed" by owner;
      "confirmed" to "shipped" by owner;
      "pending" to "cancelled" by customer where customer_did is peer_did;
    }

    on update set updated_at from clock;
    on delete reject;
  }

  // Layer declarations
  layer orders as list for Order {
    path "orders/{id}";
    namespace creator;
    grant explicit;
    dynamic by customer_did;
    create allow customer;
    discover on sync;
    shard by day;
    retain 90;

    allow owner to read, write, sync, grant, revoke;
    allow customer to read, write, sync where customer_did is peer_did;

    broadcast to owner;
    broadcast to customer;

    order by created_at descending;
  }

  // Derived layers (incremental — hook receives change + current state)
  derive order_stats as map {
    from orders;
    using "hooks.update_order_stats";
  }

  // Relay (cross-layer side effects)
  relay on orders insert using "hooks.notify_owner";

  // Lua fallback validation
  validate orders using "validation.check_order";

  // Permit functions
  permit customer_permit for customer {
    allow read, write on orders where customer_did is peer_did;
    allow sync on orders;
    issue for customer;
  }

  permit owner_permit for owner {
    allow read, write, grant, revoke on orders;
    allow sync on orders;
    issue for customer, owner;
  }

  // UI apps
  ui_app "Shop Owner" {
    entry "shop-owner/app.lua";
    allow owner;
  }

  ui_app "Shop Customer" {
    entry "shop-customer/app.lua";
    allow customer;
  }
}
```

## 5. Grammar Specification (EBNF)

```ebnf
(* ── Top Level ── *)
file              = "app" , string , "version" , string , "{" , { decl } , "}" ;

decl              = role_decl
                  | sthithi_decl
                  | layer_decl
                  | derive_decl
                  | relay_decl
                  | validate_decl
                  | permit_decl
                  | app_decl ;

(* ── Roles ── *)
role_decl         = "role" , ident , [ "inherits" , ident ] ,
                    [ "can" , cap_list ] , ";" ;
cap_list          = capability , { "," , capability } ;
capability        = "share" | "delegate" | "relay"
                  | "accept_publish" | "manage_access" ;

(* ── Entity Schema (Sthithi shape) ── *)
sthithi_decl      = "sthithi" , ident , "{" ,
                      { field_decl | transition_block | entity_rule } ,
                    "}" ;

field_decl        = "field" , ident , { field_modifier } , ";" ;
field_modifier    = "required"
                  | "immutable"
                  | "default" , literal
                  | "from" , source_ref ;
source_ref        = "clock" | "peer_did" | "peer_role" ;

transition_block  = "transitions" , ident , "{" ,
                      { transition_rule } ,
                    "}" ;
transition_rule   = string , "to" , string , "by" , role_ref_list ,
                    [ "where" , predicate_expr ] , ";" ;

entity_rule       = "on" , entity_event , entity_action , ";" ;
entity_event      = "update" | "delete" | "child" , "insert" ;
entity_action     = "reject"
                  | "set" , ident , source_expr ;
source_expr       = "from" , source_ref
                  | "to" , literal ;

(* ── Layers ── *)
layer_decl        = "layer" , ident , "as" , layer_kind ,
                    [ "for" , ident ] ,                     (* entity binding *)
                    "{" , { layer_stmt } , "}" ;

layer_stmt        = path_stmt | namespace_stmt | grant_stmt
                  | dynamic_stmt | create_stmt | discover_stmt
                  | shard_stmt | retain_stmt | cache_stmt
                  | access_stmt | broadcast_stmt | order_stmt ;

layer_kind        = "map" | "list" | "text" | "counter" | "blob" ;

path_stmt         = "path" , string , ";" ;
namespace_stmt    = "namespace" , ( "shared" | "creator" ) , ";" ;
grant_stmt        = "grant" , ( "open" | "explicit" | "role_scoped" ) , ";" ;

dynamic_stmt      = "dynamic" , "by" , ident , ";" ;
create_stmt       = "create" , "allow" , role_ref_list , ";" ;
discover_stmt     = "discover" , "on" , discover_mode , ";" ;
discover_mode     = "sync" | "grant" ;

shard_stmt        = "shard" , "by" , resolution , ";" ;
resolution        = "minute" | "hour" | "day" | "week" | "month" ;

retain_stmt       = "retain" , int , ";" ;       (* days *)

cache_stmt        = "cache" , cache_spec , ";" ;
cache_spec        = cache_item , { "," , cache_item } ;
cache_item        = "ui_window" , "(" , int , ")"
                  | "memory_lru" , "(" , int , ")"
                  | "sync_mode" , "(" , ( "full_snapshot" | "incremental" ) , ")" ;

access_stmt       = "allow" , role_ref_list , "to" , action_list ,
                    [ "on" , scope_expr ] ,
                    [ "where" , predicate_expr ] , ";" ;

broadcast_stmt    = "broadcast" , broadcast_target ,
                    [ "debounce" , int ] , ";" ;
broadcast_target  = "all" | "granted"
                  | "to" , role_ref_list ;

order_stmt        = "order" , "by" , ident ,
                    ( "ascending" | "descending" ) , ";" ;

(* ── Derive (Lua hook with incremental contract) ── *)
derive_decl       = "derive" , ident , "as" , layer_kind , "{" ,
                      "from" , ident , ";" ,
                      [ "cache" , cache_spec , ";" ] ,
                      [ "using" , string , ";" ] ,
                    "}" ;

(* ── Relay (cross-layer side effects) ── *)
relay_decl        = "relay" , "on" , ident , relay_event ,
                    "using" , string , ";" ;
relay_event       = "insert" | "update" | "delete" ;

(* ── Validate (Lua fallback) ── *)
validate_decl     = "validate" , ident , "using" , string , ";" ;

(* ── Permit Functions ── *)
permit_decl       = "permit" , ident , "for" , role_ref_list , "{" ,
                      { permit_stmt } ,
                    "}" ;
permit_stmt       = permit_allow | permit_issue ;
permit_allow      = "allow" , action_list , "on" , ident ,
                    [ "where" , predicate_expr ] , ";" ;
permit_issue      = "issue" , "for" , role_ref_list , ";" ;

(* ── UI Apps ── *)
app_decl          = "ui_app" , string , "{" ,
                      "entry" , string , ";" ,
                      "allow" , role_ref_list , ";" ,
                    "}" ;

(* ── Shared Productions ── *)
role_ref_list     = ident , { "," , ident } ;
action_list       = action , { "," , action } ;
action            = "create" | "read" | "write" | "sync"
                  | "grant" | "revoke" ;
scope_expr        = ident | "*" ;

predicate_expr    = predicate , { "and" , predicate } ;
predicate         = ident , "is" , value_ref
                  | ident , "is" , "not" , value_ref
                  | ident , "in" , "(" , value_list , ")"
                  | ident , "above" , value_ref
                  | ident , "below" , value_ref ;
value_ref         = "peer_did" | "peer_role" | string | int | "true" | "false" ;
value_list        = value_ref , { "," , value_ref } ;

literal           = string | int | float | "true" | "false" | "null" ;
string            = '"' , { char } , '"' ;
ident             = letter , { letter | digit | "_" } ;
int               = digit , { digit } ;
float             = digit , { digit } , "." , digit , { digit } ;
```

## 6. Sthithi Declaration

A `sthithi` block declares the shape of data stored in a layer. It is NOT a type annotation — it is a set of constraints the runtime enforces.

### 6.1 Fields

```osv
sthithi Order {
  field status default "pending";
  field total required;
  field customer_did from peer_did immutable;
  field created_at from clock immutable;
  field updated_at from clock;
  field notes;
}
```

| Modifier | Meaning |
|----------|---------|
| `required` | `Parivarta` without this field is rejected |
| `immutable` | Field cannot be changed after first write |
| `default <literal>` | Value auto-populated if absent on insert |
| `from clock` | Value auto-populated from `ClockSource::now_unix()` |
| `from peer_did` | Value auto-populated from the acting peer's DID |
| `from peer_role` | Value auto-populated from the acting peer's role |

A field with no modifiers is optional and freely mutable.

### 6.2 State transitions

```osv
transitions status {
  "pending" to "confirmed" by owner;
  "confirmed" to "shipped" by owner;
  "shipped" to "delivered" by owner, customer;
  "pending" to "cancelled" by customer where customer_did is peer_did;
}
```

- The field named in `transitions <field>` must be declared in the same `sthithi`.
- Any `Parivarta` that changes this field must match exactly one transition rule.
- `by <roles>` constrains who can trigger the transition.
- `where <predicate>` adds field-level conditions.
- **CRDT safety**: transitions resolve via LWW. If two peers race `pending -> confirmed` and `pending -> cancelled`, the last-write wins. The grammar does not prevent this — it is the app developer's responsibility to design non-conflicting transitions (e.g., only the owner can confirm, only the customer can cancel).

### 6.3 Entity rules

```osv
on update set updated_at from clock;
on delete reject;
on child insert set item_count to item_count + 1;  // future: arithmetic
```

Entity rules are side effects that fire declaratively. `on delete reject` makes the entity append-only (deletes are hard-rejected). `on update set ...` auto-populates fields on every mutation.

## 7. Layer Declarations

### 7.1 Entity binding

```osv
layer orders as list for Order { ... }
```

`for Order` binds the layer to a `sthithi` declaration. Every item in the list must conform to the `Order` schema. Without `for`, the layer is untyped (raw CRDT).

### 7.2 Dynamic layers

Current state: dynamic layers are created via Lua string manipulation (`channels.lua:ensure_channel_shard`, `scribe:create_layer`). Path templates use regex matching.

v2 replaces this with declarative syntax:

```osv
layer orders as list for Order {
  path "orders/{id}";
  namespace creator;
  dynamic by customer_did;
  create allow customer;
  discover on sync;
}
```

| Statement | Meaning |
|-----------|---------|
| `dynamic by <field>` | Layer instances are parameterized by this field. Each unique value creates a separate CRDT document. |
| `create allow <roles>` | These roles can create new dynamic layer instances. |
| `discover on sync` | New instances are discovered during sync (peer announces layers). |
| `discover on grant` | New instances are discovered only via explicit grant. |

The runtime resolves `{id}` from the dynamic parameter. No Lua path parsing needed.

### 7.3 Broadcast policy

Current state: all changes broadcast to all subscribers unconditionally.

```osv
broadcast all;                              // broadcast everything (current default)
broadcast granted;                          // only to peers with explicit grant
broadcast to owner;                         // only this role
broadcast to customer debounce 500;         // role + debounce in milliseconds
```

Broadcast controls **who** receives CRDT updates and **when** (debounce). It does not support field-level filtering — CRDT updates are opaque binary blobs and stripping fields would break merge semantics. If a peer syncs a layer, they get the full data. Field-level visibility is a UI concern, not a sync concern.

Broadcast statements are per-layer. Multiple statements combine (each subscriber gets updates if any rule matches).

### 7.4 Ordering

```osv
order by created_at descending;
```

Declares the default sort order for list layers. The runtime maintains a sorted index. Lua's `scribe:bind` and the UI binding system respect this ordering without manual `table.sort`.

### 7.5 Retention

```osv
retain 90;  // days
```

For time-sharded layers, `retain` declares how long old shards are kept. After the retention period, shards are evicted from storage and no longer synced.

## 8. Permit Model

### 8.1 Permits are signed executable functions

This is the foundational principle: **permits are NOT string roles — they are signed executable functions** that declare both authorization AND mutation behavior.

A role is a compiler convenience. At compile time, roles are expanded into per-role function sets. At runtime, a permit is a signed bundle of functions. The node is a function executor.

### 8.2 Permit blocks

```osv
permit customer_permit for customer {
  allow read, write on orders where customer_did is peer_did;
  allow sync on orders;
  issue for customer;
}

permit owner_permit for owner {
  allow read, write, grant, revoke on orders;
  allow sync on orders;
  issue for customer, owner;
}
```

### 8.3 Delegation via `issue for`

`issue for <roles>` declares that the holder of this permit can issue sub-permits for the listed roles. Sub-permits are constrained:

- A sub-permit can only contain functions that are a **subset** of the parent's functions.
- The parent's `where` predicates are inherited — a sub-permit cannot widen access.
- Delegation depth is tracked. Each `issue` increments the depth counter. The runtime enforces a maximum delegation depth (configurable per app, default 3).

Example chain:
1. Owner holds `owner_permit` (issued by the app at page creation).
2. Owner issues `customer_permit` to Alice (allowed because `issue for customer`).
3. Alice's `customer_permit` contains `issue for customer`, so she can delegate to Bob.
4. Bob's sub-permit is depth 3 — further delegation is rejected.

### 8.4 Lowering

The compiler lowers permit blocks into `PolicyRule` entries with conditions. The `delegation_rules` vec (currently empty) gets populated from `issue for` declarations:

```rust
DelegationRule {
    from: Subject::Role { name: "owner" },
    to: Subject::Role { name: "customer" },
    actions: vec![Read, Write, Sync],
    resource: ResourceSelector::Layer { name: "orders" },
    max_depth: 3,
    no_escalation: true,
    condition: None,
}
```

## 9. Runtime Types

### 9.1 Sthithi

Replaces `serde_json::Value` as the canonical value representation:

```rust
pub enum Sthithi {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<Sthithi>),
    Map(Vec<(String, Sthithi)>),
}
```

Key differences from `serde_json::Value`:

| Problem with `serde_json::Value` | `Sthithi` solution |
|----------------------------------|-------------------|
| `Number` conflates int and float | Separate `Int(i64)` and `Float(f64)` |
| No binary type (base64 hack) | `Bytes(Vec<u8>)` |
| Empty array/object ambiguity in Lua | `List` vs `Map` are distinct |
| `Map` uses `BTreeMap<String, _>` | `Vec<(String, _)>` preserves insertion order |

Conversion traits:

```rust
impl From<LoroValue> for Sthithi { ... }      // lossless
impl From<&Sthithi> for LoroValue { ... }     // lossless
impl From<Sthithi> for LuaValue { ... }       // direct, no JSON hop
impl From<LuaValue> for Sthithi { ... }       // direct, no JSON hop
impl From<&Sthithi> for SlintValue { ... }    // direct, no JSON hop
```

The `serde_json::Value` conversion functions in `domains/src/layer.rs`, `scribe/src/loro_observer.rs`, `lua_runtime/src/bindings/convert.rs`, and `renderer_slint/src/value_convert.rs` are replaced by `Sthithi` conversions. The 3-hop path becomes a 1-hop path:

```
Before: LoroValue -> json -> LuaValue -> json -> SlintValue
After:  LoroValue -> Sthithi -> LuaValue (or SlintValue)
```

### 9.2 Parivarta

Replaces `JsonOp` as the canonical change representation:

```rust
pub struct Parivarta {
    /// Layer this change targets
    pub layer: String,

    /// Operation kind
    pub op: OpKind,

    /// Path within the layer (e.g., "root/items")
    pub path: String,

    /// Key for map operations
    pub key: Option<String>,

    /// Index for list operations
    pub index: Option<usize>,

    /// New value
    pub value: Option<Sthithi>,

    /// Previous value (for updates, looked up from Sthithi)
    pub old_value: Option<Sthithi>,

    /// Intent tag for state transitions (e.g., "confirm", "cancel")
    pub intent: Option<String>,

    /// Source peer (user_did, device_id). None = local.
    pub from_peer: Option<(String, String)>,
}

pub enum OpKind {
    Insert,
    Update,
    Delete,
    Set,
}
```

`Parivarta` carries the `intent` field for state transitions. When a peer changes `status` from `"pending"` to `"confirmed"`, the intent is matched against the transition rules in the `sthithi` block. The runtime validates:

1. The current value of the transition field matches the `from` state.
2. The `to` state matches the new value.
3. The peer's role is in the `by` list.
4. All `where` predicates pass.

If validation fails, the `Parivarta` is hard-rejected.

## 10. Validation Pipeline

### 10.1 Current pipeline

```
Peer update bytes
  -> Layer::extract_ops_from_bytes() (empty temp LoroDoc + subscribe)
  -> Vec<JsonOp>
  -> mpsc channel to kunki ValidationService
  -> spawn LuaRuntime per request
  -> page.lua validate(ops) runs in Lua
  -> accept/reject
```

Problems:
- Spawning a `LuaRuntime` per validation request is heavyweight.
- The empty temp doc cannot capture delete operations (documented limitation).
- Validation logic is split between grammar (access rules) and Lua (field checks).

### 10.2 v2 pipeline

```
Peer update bytes
  -> Layer::extract_parivarta() (same technique, produces Vec<Parivarta>)
  -> Grammar validation (schema, transitions, predicates — pure Rust)
  -> If validate_decl exists: Lua fallback validation (reuse existing runtime)
  -> accept/reject
  -> Apply to layer, fire entity_rules, update inline aggregations
```

Grammar-declared constraints (required fields, immutability, transitions, `where` predicates) are checked in pure Rust — no Lua spawn needed. The `validate <layer> using "hook"` declaration is the escape hatch for complex validation that cannot be expressed in the grammar.

## 11. Relay and Cross-Layer Effects

```osv
relay on orders insert using "hooks.notify_owner";
relay on orders update using "hooks.update_inventory";
```

A `relay` fires a Lua hook when a specific event occurs on a layer. Unlike entity rules (which are declarative side effects on the same entity), relays are for cross-layer effects:

- Inserting an order creates a notification in a different layer.
- Updating inventory recalculates a dashboard aggregate.

Relays fire **after** validation and **after** the change is applied. They run in the existing Lua runtime (no new spawn). They receive the `Parivarta` as input and can call `scribe:insert`, `scribe:update`, etc. on other layers.

## 12. Derived Layers

### 12.1 Incremental derivation contract

All derivations use Lua hooks. The grammar declares **that** a derivation exists and **what** triggers it. The **how** is always a Lua function.

```osv
derive order_stats as map {
  from orders;
  using "hooks.update_order_stats";
}
```

**v0 contract** (full recompute): the hook receives the full source layer content and returns the full derived layer content. Simple but expensive — recomputes everything on every change.

**v2 contract** (incremental): the hook receives the `Parivarta` (the change that just happened on the source layer) and the current `Sthithi` of the derived layer. It returns the updated derived state.

```lua
-- hooks.lua
function update_order_stats(parivarta, current)
  current = current or { count = 0, total_revenue = 0 }

  if parivarta.op == "insert" then
    current.count = current.count + 1
    current.total_revenue = current.total_revenue + parivarta.value.total
  elseif parivarta.op == "delete" then
    current.count = current.count - 1
    current.total_revenue = current.total_revenue - parivarta.old_value.total
  elseif parivarta.op == "update" then
    local diff = parivarta.value.total - parivarta.old_value.total
    current.total_revenue = current.total_revenue + diff
  end

  return current
end
```

The contract:

| Argument | Type | Meaning |
|----------|------|---------|
| `parivarta` | `Parivarta` (Lua table) | The change on the source layer |
| `current` | `Sthithi` (Lua table or nil) | Current state of the derived layer. `nil` on first call. |
| **return** | `Sthithi` (Lua table) | New state for the derived layer |

This replaces the need for arithmetic in the grammar. Counters, sums, averages, distinct counts — all computed incrementally in Lua. The grammar stays small; the derivation logic is explicit and visible.

### 12.2 Why not built-in aggregates?

We considered adding grammar-level aggregates (`derive X as count`, `derive X as sum of field`). We rejected this because:

1. **The grammar should declare behavior, Lua should compute it.** Aggregates are computation, not declaration.
2. **Built-in aggregates are a closed set.** Every new aggregate type (weighted average, percentile, running median) would require a grammar change. Lua hooks handle any computation.
3. **Incremental Lua hooks are explicit.** The developer sees exactly how the derived value is maintained. No hidden runtime behavior.

### 12.3 Bootstrap / recovery

When a derived layer is empty (first load, or data loss), the runtime calls the hook once per existing item in the source layer with synthetic `insert` Parivartas. This rebuilds the derived state from scratch using the same incremental logic — no separate "full recompute" code path needed.

## 13. Compiler Output

The `osv_decl` compiler produces `CompiledArtifacts` with expanded sections:

```rust
pub struct CompiledArtifacts {
    pub permit: PermitArtifact,
    pub runtime: RuntimeArtifact,
    pub schema: SchemaArtifact,       // NEW
    pub validation: ValidationArtifact, // NEW
}

pub struct SchemaArtifact {
    /// Entity schemas keyed by name
    pub entities: HashMap<String, EntitySchema>,
    /// Layer-to-entity bindings
    pub layer_bindings: HashMap<String, String>,
}

pub struct EntitySchema {
    pub fields: Vec<FieldSchema>,
    pub transitions: Vec<TransitionSchema>,
    pub entity_rules: Vec<EntityRuleSchema>,
}

pub struct FieldSchema {
    pub name: String,
    pub required: bool,
    pub immutable: bool,
    pub default: Option<Sthithi>,
    pub source: Option<FieldSource>,
}

pub enum FieldSource {
    Clock,
    PeerDid,
    PeerRole,
}

pub struct TransitionSchema {
    pub field: String,
    pub from: String,
    pub to: String,
    pub allowed_roles: Vec<String>,
    pub predicates: Vec<PredicateSchema>,
}

pub struct ValidationArtifact {
    /// Layers with Lua fallback validation hooks
    pub lua_validators: HashMap<String, String>,
    /// Derivation definitions (source layer -> derived layer + Lua hook)
    pub derivations: Vec<DerivationDef>,
    /// Relay hooks
    pub relays: Vec<RelayDef>,
    /// Broadcast policies per layer
    pub broadcasts: HashMap<String, Vec<BroadcastPolicy>>,
}
```

The `PermitArtifact` is extended with populated `delegation_rules` from `issue for` declarations.

## 14. Migration from v0

### 14.1 What changes

| v0 construct | v2 equivalent |
|-------------|---------------|
| `role X can ...;` | Same (no change) |
| `layer X as kind { ... }` | Same + optional `for Entity` binding |
| `path`, `namespace`, `grant`, `shard`, `cache` | Same (no change) |
| `allow X to actions;` | Same + optional `where predicate` |
| `derive X as kind { from ...; using ...; }` | Same syntax, new contract: hook receives `(parivarta, current)` not full data |
| `ui_app` | Same (no change) |
| — | NEW: `sthithi` blocks |
| — | NEW: `transitions` blocks |
| — | NEW: `on event action` entity rules |
| — | NEW: `dynamic by`, `create allow`, `discover on` |
| — | NEW: `broadcast` statements |
| — | NEW: `order by` statements |
| — | NEW: `relay` declarations |
| — | NEW: `validate` declarations |
| — | NEW: `permit` blocks with `issue for` |

### 14.2 What Lua code gets replaced

| Current Lua pattern | v2 replacement |
|---------------------|---------------|
| `page.lua` field validation | `sthithi` field modifiers (`required`, `immutable`) |
| `ORDER_STATES` table + guards | `transitions` block |
| `os.time()` for timestamps | `from clock` field modifier |
| `channels.lua:ensure_channel_shard` | `dynamic by`, `create allow`, `discover on` |
| Manual `table.sort` in bindings | `order by` |
| Full-recompute derivation hooks | Incremental derivation contract (`parivarta` + `current`) |
| Implicit broadcast-everything | `broadcast` statements |

### 14.3 What stays in Lua

- Complex validation logic (via `validate X using "hook"`)
- Cross-layer side effects (via `relay on X event using "hook"`)
- Derivation transforms (via `derive X { using "hook" }`)
- UI event handlers and app logic
- Anything the grammar cannot express

## 15. Example: Group Chat (v2)

```osv
app "group-chat" version "2.0.0" {
  role owner can share, delegate, accept_publish, manage_access;
  role node can relay, share, accept_publish, manage_access;
  role collaborator can manage_access;
  role layer_authority can manage_access;

  sthithi Message {
    field text required;
    field sender_did from peer_did immutable;
    field sent_at from clock immutable;
    field edited_at from clock;
    field thread_id;

    on update set edited_at from clock;
    on delete reject;
  }

  sthithi Channel {
    field name required;
    field created_by from peer_did immutable;
    field created_at from clock immutable;
    field description;
  }

  layer app_data as map {
    path "app:Group Chat";
    namespace shared;
    allow owner to read, write, sync;
    allow node, collaborator, layer_authority to read, sync;
  }

  layer presence as map {
    path "presence";
    namespace shared;
    allow owner, node, collaborator, layer_authority to read, write, sync;
    broadcast all debounce 1000;
  }

  layer channels as map for Channel {
    path "channels";
    namespace shared;
    allow owner, node to read, write, sync;
    allow collaborator, layer_authority to read, sync;
    broadcast all;
  }

  layer channel_messages as list for Message {
    path "channels/{id}/messages";
    namespace shared;
    dynamic by id;
    create allow owner, node, collaborator;
    discover on sync;
    shard by day;
    retain 365;

    allow owner, node, collaborator, layer_authority to read, write, sync;

    broadcast all;
    order by sent_at descending;
  }

  layer dm_messages as list for Message {
    path "dms/{id}/messages";
    namespace creator;
    grant explicit;
    dynamic by id;
    create allow owner, collaborator;
    discover on grant;

    allow owner, node, collaborator, layer_authority to read, write, sync;
    allow owner, layer_authority to grant, revoke;

    broadcast granted;
    order by sent_at descending;
  }

  // Derived layer: message stats maintained incrementally
  derive channel_stats as map {
    from channel_messages;
    using "hooks.update_channel_stats";
  }

  permit collaborator_permit for collaborator {
    allow read, write, sync on channel_messages;
    allow read, write, sync on dm_messages;
    allow read, sync on channels;
    allow read, write, sync on presence;
    allow read, sync on app_data;
    issue for collaborator;
  }

  permit owner_permit for owner {
    allow read, write, sync, grant, revoke on channel_messages;
    allow read, write, sync, grant, revoke on dm_messages;
    allow read, write, sync on channels;
    allow read, write, sync on presence;
    allow read, write, sync on app_data;
    issue for collaborator, owner;
  }

  ui_app "Group Chat" {
    entry "group-chat/app.lua";
    allow owner, collaborator, node;
  }
}
```

## 16. Implementation Plan

### Phase 1: Runtime types (`Sthithi` and `Parivarta`)

**Goal**: Replace `serde_json::Value` on the data path.

1. Add `Sthithi` and `Parivarta` types to `domains/src/`.
2. Implement `From<LoroValue> for Sthithi` and `From<&Sthithi> for LoroValue`.
3. Implement direct `Sthithi -> LuaValue` and `LuaValue -> Sthithi` conversions in `lua_runtime`.
4. Implement direct `Sthithi -> SlintValue` conversion in `renderer_slint`.
5. Migrate `Layer::to_json_value()` to `Layer::to_sthithi()`.
6. Migrate `JsonOp` to `Parivarta` in `Layer::extract_ops_from_bytes`.
7. Update `scribe/src/loro_observer.rs` to use `Sthithi` instead of `loro_value_to_json`.
8. Remove duplicate conversion functions.

**Verification**: All existing integration tests pass. The 3-hop conversion path is replaced by direct conversions.

### Phase 2: Grammar extensions (parser + AST)

**Goal**: Parse the new constructs.

1. Extend `osv_decl/src/ast.rs` with `SthithiDecl`, `FieldDecl`, `TransitionBlock`, `EntityRule`, `PermitDecl`, `RelayDecl`, `ValidateDecl`, `BroadcastStmt`, `OrderStmt`, `LayerDeriveStmt`.
2. Extend `osv_decl/src/lexer.rs` with new keywords.
3. Extend `osv_decl/src/parser.rs` to parse the new constructs.
4. Extend `osv_decl/src/semantic.rs` with validation (entity references exist, transition fields exist, `for Entity` references valid entity, etc.).

**Verification**: Parser round-trips the group-chat v2 example. Semantic validation catches invalid references.

### Phase 3: Compiler lowering

**Goal**: Produce `SchemaArtifact` and `ValidationArtifact`.

1. Extend `osv_decl/src/lowering.rs` to produce `SchemaArtifact` and `ValidationArtifact`.
2. Populate `delegation_rules` from `issue for` declarations.
3. Lower `broadcast` statements to `BroadcastPolicy`.
4. Lower inline aggregations to `AggregationDef`.

**Verification**: Lowered output for group-chat v2 matches expected structure.

### Phase 4: Runtime enforcement

**Goal**: The Scribe actor uses compiled schemas at runtime.

1. Schema validation in `scribe/src/sync/apply.rs`: check `Parivarta` against `EntitySchema` before applying.
2. Auto-populate `from clock`, `from peer_did`, `from peer_role` fields.
3. Enforce `immutable` on updates.
4. Enforce `required` on inserts.
5. Match state transitions against `TransitionSchema`.
6. Fire entity rules after apply.
7. Trigger incremental derivations (pass `Parivarta` + current derived state to Lua hook).
8. Apply broadcast policies in `scribe/src/loro_observer.rs`.

**Verification**: Integration tests cover: required field missing (rejected), immutable field changed (rejected), valid transition (accepted), invalid transition (rejected), auto-populated fields correct, incremental derivation produces correct results.

### Phase 5: Dynamic layers (grammar-driven)

**Goal**: Replace Lua string manipulation with grammar-driven layer creation.

1. Runtime reads `dynamic by`, `create allow`, `discover on` from compiled artifacts.
2. Layer creation API checks `create allow` roles.
3. Discovery mode controls whether layers appear on sync or only via grant.
4. Remove `ensure_channel_shard` and manual path parsing from Lua.

**Verification**: Group-chat channel creation works via grammar without Lua path manipulation.

### Phase 6: Permit delegation

**Goal**: `issue for` works end-to-end.

1. `gurkha` reads `DelegationRule` from `PolicyFacts`.
2. Permit issuance checks: issuer holds parent permit, target role is in `issue for` list, sub-permit functions are subset of parent.
3. Delegation depth is tracked and enforced.

**Verification**: Owner issues customer permit (accepted), customer issues sub-permit (accepted up to depth limit), customer attempts to issue owner permit (rejected — not in `issue for`).

### Phase 7: Sample app migration

**Goal**: All sample apps use v2 grammar.

1. Migrate `group-chat/app.osv` to v2.
2. Remove replaced Lua validation/state/path code.
3. Migrate `my-shop/app.osv` (if layers are added).
4. Migrate `my-booking/app.osv` (if layers are added).

**Verification**: All sample apps work identically to v0 behavior but with less Lua code.

## 17. Open Questions

1. **Conflict resolution for transitions.** LWW is the only CRDT resolution policy. If two peers race `pending -> confirmed` and `pending -> cancelled`, last-write wins. The `by <role>` separation already prevents most conflicts. Adding priority resolution would require custom merge functions that Loro doesn't natively support. Current stance: LWW only, design around it with non-overlapping role assignments.
2. **OCaml parser.** The grammar is ~50 rules. When it exceeds ~100 rules or needs formal verification, revisit moving the parser to OCaml with Rust FFI.

### Resolved

- **Arithmetic in entity rules.** No arithmetic in the grammar. Counters, sums, and all computed values are handled by incremental derivation hooks in Lua (see section 12). The grammar declares the derivation trigger; Lua computes the value.
- **Schema migration.** Breaking changes are acceptable. Version bump = new page. No migration mechanism needed.
- **Broadcast field filtering.** Dropped `select` from broadcast. CRDT updates are opaque binary blobs — stripping fields would break merge semantics. Broadcast controls who and when (role + debounce), not what fields. Field-level visibility is a UI concern.
