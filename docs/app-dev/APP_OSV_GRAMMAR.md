# app.osv Grammar (v0)

This document defines the currently implemented grammar for `app.osv` in `osv_decl`.

`app.osv` is the declaration source for app roles, layers, derivations, and UI app entrypoints.

## Design Goals

- One declaration file per app package
- English-like tokens with strict structural grammar
- Compile-time validation before runtime
- Offline-first safe semantics (no network request-response in declarations)

## Top-Level Structure

```osv
app "My App" version "0.1.0" {
  role owner can share, delegate;

  layer orders as list {
    path "orders/{id}";
    namespace shared;
    grant explicit;
    allow owner to create,read,write,sync,grant,revoke on *;
  }

  derive orders_summary as map {
    from orders;
    using "rules.summarize_orders";
  }

  ui_app "Orders" {
    entry "orders/app.lua";
    allow owner;
  }
}
```

## EBNF

```ebnf
file            = "app", string, "version", string, "{", { decl }, "}" ;

decl            = role_decl | layer_decl | derive_decl | app_decl ;

role_decl       = "role", ident, [ "inherits", ident ], [ "can", cap_list ], ";" ;
cap_list        = capability, { ",", capability } ;
capability      = "share" | "delegate" | "relay" | "accept_publish" | "manage_access" ;

layer_decl      = "layer", ident, "as", layer_kind, "{",
                    "path", string, ";",
                    "namespace", namespace, ";",
                    [ "grant", grant_mode, ";" ],
                    [ "shard", "by", resolution, ";" ],
                    [ "cache", cache_spec, ";" ],
                    { access_decl },
                  "}" ;

layer_kind      = "map" | "list" | "text" | "counter" | "blob" ;
namespace       = "shared" | "creator" ;
grant_mode      = "open" | "explicit" | "role_scoped" ;

cache_spec      = cache_item, { ",", cache_item } ;
cache_item      = "ui_window", "(", int, ")"
                | "memory_lru", "(", int, ")"
                | "sync_mode", "(", ("full_snapshot" | "incremental"), ")"
                | "retention_days", "(", int, ")" ;

access_decl     = "allow", role_ref_list, "to", action_list, [ "on", scope_expr ], ";" ;
role_ref_list   = ident, { ",", ident } ;
action_list     = action, { ",", action } ;
action          = "create" | "read" | "write" | "sync" | "grant" | "revoke" ;
scope_expr      = ident | "*" ;

derive_decl     = "derive", ident, "as", layer_kind, "{",
                    "from", ident, ";",
                    [ "cache", cache_spec, ";" ],
                    [ "using", string, ";" ],
                  "}" ;

app_decl        = "ui_app", string, "{",
                    "entry", string, ";",
                    "allow", role_ref_list, ";",
                  "}" ;
```

## Semantic Validation (Implemented)

- Names must be unique:
  - role names
  - layer names
  - `ui_app` names
- References must exist:
  - `inherits` target role
  - roles in `allow` rules
  - roles in `ui_app allow`
  - `derive from` source layer
- Role inheritance cannot contain cycles
- Sharding invariants:
  - `shard by ...` requires `{period}` in layer path
  - `{period}` in path requires `shard by ...`
- Cache invariants:
  - `retention_days(...)` requires time-sharded layer
- Explicit grant invariant:
  - `grant explicit` requires at least one `allow` action with `grant` or `revoke`

## Compiler Output (Current)

`compile_source()` produces a `CompiledArtifacts` value with:

- `permit.roles`
- `permit.osv_policy`
  - `roles`
  - `policy_rules` (normalized allow rules lowered from layer access declarations)
  - `delegation_rules` (currently empty in v0)
  - `dynamic_layer_schemas`
- `permit.dynamic_layer_schemas[]`
  - name, path, namespace, grant, storage strategy, resolution
- `runtime.app_name`, `runtime.app_version`
- `runtime.ui_apps[]`

## v0 Scope Notes

- This is a minimal vertical slice.
- Access, validation, and derivation blocks are not fully implemented yet.
- Lowering currently emits internal artifacts, not final runtime wiring.
