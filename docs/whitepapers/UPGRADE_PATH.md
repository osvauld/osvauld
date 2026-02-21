# Osvauld Verification Suite & Paper Series — Upgrade Path

## What We Are Writing

These are not traditional whitepapers. Traditional whitepapers make claims.
We are writing **formal behavioral specifications with executable proofs**.

The distinction:

| | Traditional whitepaper | What we produce |
|---|---|---|
| Claim | "AI agents have scoped access" | Property: `ai_agent.can_access("presence", "write") == false` |
| Evidence | "See our architecture diagram" | `osvauld-verify static specs/wp8_ai_participant.osv` produces PASS |
| Reproducibility | "Trust us" | `cargo run --bin osvauld-verify -- all` |
| When implementation changes | Paper becomes stale | Verification fails — paper stays honest |

### Three Artifacts

**Artifact 1: Formal Specification**

The `.osv` grammar IS the specification language. When you write:

```osv
allow ai_agent to read, sync on presence;
```

That is a formal statement: "the ai_agent role SHALL have read and sync access
to the presence layer, and SHALL NOT have write access." Declarative, precise,
machine-parseable.

**Artifact 2: Executable Proof**

The `osvauld-verify` binary compiles the spec, runs it through the full pipeline
(compile, sign, delegate, parse, decide), and checks that the implementation's
decisions match the specification's intent. If they match, the property holds.

**Artifact 3: Verification Report**

The output is a structured proof transcript. Anyone can run `osvauld-verify` and
get the same results. The paper is the human-readable explanation of what the
transcript proves.

### What the Paper Is

The paper is the narrative that connects the specification to its implications:

- **Why does this property matter?** (motivation)
- **How is it declared?** (the `.osv` spec)
- **How is it enforced?** (the pipeline)
- **What does the proof show?** (the verification output)
- **What does this guarantee for users/builders?** (implications)

Papers present verified properties, not claims. Each paper follows:

1. Property statement (precise, testable)
2. Declaration (`.osv` grammar or protocol construct)
3. Proof (verification output showing PASS)
4. Implications (what this guarantees)

### What is Novel

Most decentralized system papers (IPFS, Dat, SSB, UCAN itself) describe
architecture and claim properties. None ship a verification binary where you
run `verify all` and get a machine-checked proof of every claim in the paper.

We publish **papers with reproducible proofs**.

### Writing Order

1. Build `osvauld-verify` and the synthetic `.osv` specs first
2. Run verification, get the report
3. Write the papers around the verification output — the output dictates the
   paper's structure, not the other way around

Papers are explanations of proofs, not claims that need evidence.

---

## Current State

| Asset | Status |
|-------|--------|
| Exploratory Report (Draft 0) | AI-generated research scaffold |
| Whitepaper Draft v1 | Monolithic draft (~560 lines) |
| Test suite | ~210 tests across 35 files |
| Integration test infra | Peer, Scenario, MockConnection, Tracer, ManualClock |
| `.osv` compiler pipeline | lexer, parser, semantic, lowering to PolicyFacts |
| Permit pipeline | PolicyFacts, UCAN token, gurkha access decision |
| DST infrastructure | None |
| Grammar test coverage | ~20% (7 of ~35 error codes) |
| AI agent role | Not defined in any app |
| External spec citations | 2 URLs (ucan.xyz, loro.dev) |

---

## Target State

| Asset | Target |
|-------|--------|
| Paper series | 8 behavioral verification documents |
| `test_harness` crate | Shared simulation kernel (extracted from integration_tests) |
| `osvauld-verify` binary | Correctness verification, benchmarks, JSON-RPC serve mode |
| Synthetic `.osv` specs | ~10 purpose-built specs for behavioral properties |
| Test suite | ~400 tests (unit, integration, e2e, grammar, verification) |
| AI agent role | Defined in group-chat app + synthetic specs |
| External spec citations | Full RFC/paper references per paper |

---

## Architecture

### Crate Structure

```
test_harness/                    # Shared simulation kernel
  Cargo.toml                     # depends on: domains, herald, gurkha, scribe,
  src/                           #   butler, courier, transport, control_server
    lib.rs
    peer.rs                      # Full-stack in-memory peer
    scenario.rs                  # Multi-peer builder
    tracer.rs                    # Message capture + assertions
    fixtures.rs                  # Wait helpers
    synthetic.rs                 # Deterministic identity + data generation
    fault.rs                     # FaultyConnection wrapper (future)

osvauld_verify/                  # Correctness verification binary
  Cargo.toml                     # depends on: test_harness, osv_decl, policy_model
  specs/                         # Synthetic .osv specs
    wp1_offline_invariant.osv
    wp2_sharding.osv
    wp2_layer_composition.osv
    wp4_role_isolation.osv
    wp4_delegation_chain.osv
    wp4_dm_isolation.osv
    wp4_dynamic_schemas.osv
    wp5_all_constructs.osv
    wp7_time_sharding.osv
    wp8_ai_participant.osv
  src/
    main.rs                      # CLI: static | dynamic | bench | serve
    report.rs                    # JSON + markdown output
    handler.rs                   # VerifyHandler: CommandHandler for in-memory peers
    static_verify/
      mod.rs
      permit_roundtrip.rs        # .osv -> token chain -> access decisions
      grammar_coverage.rs        # All error codes, all constructs
    dynamic_verify/
      mod.rs
      offline_merge.rs           # Offline write + convergence
      sync_protocol.rs           # Handshake, 3-step sync, late joiner
      shard_convergence.rs       # Time-sharded concurrent writes
      ai_scenario.rs             # AI agent in group chat
    bench/
      mod.rs
      chat_throughput.rs         # Messages/second between peers
      sync_latency.rs            # Write-to-convergence time
      permit_cost.rs             # Token signing throughput
      merge_cost.rs              # CRDT merge time at scale

integration_tests/               # Refactored to use test_harness
  Cargo.toml                     # depends on: test_harness
  src/
    lib.rs
    tests/                       # Existing tests (unchanged logic)

docs/whitepapers/
  00_EXPLORATORY_ENGINEERING_REPORT.md
  UPGRADE_PATH.md                # This document
  01_SYSTEM_THESIS.md
  02_DATA_MODEL.md
  03_PROTOCOL.md
  04_CAPABILITY_SECURITY.md
  05_RUNTIME.md
  06_OBSERVABILITY.md
  07_TIME_MODELING.md
  08_AI_PARTICIPANT.md
```

### Data Flow: Specification to Proof

```
.osv spec (synthetic or real app)
    |
    |  [osv_decl::compile_source]
    |    lexer -> tokens -> parser -> AST -> semantic::check -> lowering::compile
    |
    v
  CompiledArtifacts
    |-- PermitArtifact
    |     '-- osv_policy: PolicyFacts    <- the formal behavioral specification
    '-- RuntimeArtifact
    |
    |  [gurkha::issue_page_owner_token_from_policy]
    |    embeds PolicyFacts as osv_policy fact in UCAN JWT
    |    builds issue_on delegation templates (node, viewer, roles)
    |
    v
  Signed owner token
    |
    |  [delegation chain]
    |    owner -> node -> each role (ai_agent, collaborator, etc.)
    |
    v
  Per-role delegated tokens
    |
    |  [gurkha::PolicyPermit::from_token]
    |    parse JWT -> extract osv_policy -> deserialize PolicyFacts
    |
    |  [gurkha::policy::decision::can_access]
    |    iterate policy_rules: action match -> resource match -> subject match
    |    deny-takes-precedence, then allow if any rule matched
    |
    v
  Access decisions per role per layer <- VERIFIED against expected behavior
```

### Four Operating Modes

#### Mode 1: `osvauld-verify static`

Compiles `.osv` specs, runs the full permit pipeline, asserts access decisions.
No network, no MockConnection needed.

Covers: WP4 (capability security), WP5 (grammar correctness), WP8 (AI agent).

#### Mode 2: `osvauld-verify dynamic`

Creates N in-memory peers with MockConnections. Runs protocol scenarios. Asserts
convergence, isolation, and message sequences via Tracer.

Covers: WP1 (offline-first), WP2 (data model), WP3 (protocol), WP7 (time).

#### Mode 3: `osvauld-verify bench`

Same infrastructure as dynamic, but measuring performance. MockConnection means
zero network latency — measures pure system cost. Results are deterministic and
comparable across runs.

| Benchmark | Measures | Inspired by |
|-----------|---------|-------------|
| `chat_throughput` | Messages/second, N msgs between 2 peers | Group chat send |
| `sync_latency` | Time from write to viewer convergence | Group chat sync |
| `shard_creation` | Time to create + sync a new day shard | Time-sharded chat |
| `offline_merge` | Time to merge M offline writes on reconnect | Offline chat |
| `permit_issuance` | Tokens signed/second for N dynamic layers | Channel creation |
| `delegation_chain` | Time for owner -> node -> viewer chain | Viewer onboarding |
| `crdt_merge` | Loro merge time for documents of size S | Layer sync |
| `multi_peer_convergence` | Time for K peers to converge on N msgs | Scaled chat |

#### Mode 4: `osvauld-verify serve`

Exposes one Unix socket per peer with the same JSON-RPC API as `control_server`.
Python's existing `ControlClient` connects to these sockets. From Python's
perspective, it is identical to talking to real sthalam/kunki instances.

```
Current e2e architecture:
  Python -> spawn sthalam/kunki in tmux -> real QUIC -> slow, non-deterministic

With osvauld-verify serve:
  Python -> osvauld-verify (single process) -> MockConnection -> fast, deterministic
```

| Property | Current (tmux) | osvauld-verify serve |
|----------|---------------|---------------------|
| Speed | 30-120s/test | 1-5s/test |
| Determinism | Non-deterministic | Fully deterministic |
| Process mgmt | tmux, PIDs, cleanup | Single process |
| Port conflicts | Unix sockets in /tmp | In-memory channels |
| Fault injection | go_offline/go_online only | Delay, drop, reorder (future) |
| CI reliability | Flaky | Reliable |

### Python E2E Integration

The `VerifyHandler` implements the same `CommandHandler` trait as sthalam's
`ShellHandler`, wired to in-memory peers:

| Method | Implementation |
|--------|---------------|
| `ping` | Returns `{status: "ok", instance: "<peer_name>"}` |
| `signup` | Creates identity from name + passphrase via Herald |
| `login` | Loads identity from Butler store |
| `eval` | Sends to Lua runtime |
| `create_space` | Butler space API |
| `import_page` | Butler page API |
| `publish_space` | Coordinator -> MockConnection to node peer |
| `get_shareable_link` | Coordinator -> generate viewer connection string |
| `go_offline` | Coordinator -> disconnect MockConnection |
| `go_online` | Coordinator -> reconnect MockConnection |
| `set_time` | ManualClock.set_unix() |
| `advance_time` | ManualClock.advance() |
| `capture_start` | CaptureHandle -> JSONL file |
| `capture_end` | CaptureHandle -> flush and close |
| `list_spaces/pages/layers` | Butler queries |

The `control_server` crate is already fully generic — trait-based
`CommandHandler`, no network dependencies. Butler takes a `RedbStore` (local
storage) and an optional `SyncEvent` channel. Everything wires up cleanly
with MockConnections.

---

## Paper Series: Behavioral Properties

### WP1: System Thesis and Architecture

**Model**: Offline-first invariant — local writes never block on network, offline
writes merge correctly on reconnect, state persists across restarts.

| Property | Declaration | Proof method |
|----------|------------|-------------|
| P1.1 Local mutation commits without network | Layer write API | Dynamic: write while disconnected, assert local data exists |
| P1.2 Offline writes survive reconnect | CRDT merge semantics | Dynamic: both peers write offline, reconnect, assert convergence |
| P1.3 Repeated disconnect/reconnect cycles preserve state | Protocol state machine | Dynamic: N cycles, assert monotonic count increase |
| P1.4 Data persists across process restart | redb storage | Dynamic: write, shutdown, restart, assert data survives |
| P1.5 3-peer divergence converges | CRDT convergence | Dynamic: 3 peers, 1 offline, all write, reconnect, all converge |
| P1.6 No request-response patterns (except GetShareableLink) | Message type classification | Static: enumerate Message variants, assert fire-and-forget |
| P1.7 Crate boundaries enforce isolation | Workspace dependency graph | Static: parse Cargo.toml, assert no forbidden deps |

### WP2: Data Model and Layer-Native Sharding

**Model**: Layer composition provides isolation, sharding, and convergence. The
`.osv` grammar expresses sharding strategy; the runtime enforces it.

| Property | Declaration | Proof method |
|----------|------------|-------------|
| P2.1 Layer CRDT ops extract correctly | Loro operations | Static: write, extract, assert op type/key/value |
| P2.2 LayerUnit tracks dirty/observer/subscriber state | LayerUnit state machine | Static: lifecycle assertions |
| P2.3 Dynamic layer paths generate correctly | Schema pattern matching | Static: pattern to path to assert match |
| P2.4 Time-sharded layers sync to viewers | `.osv` shard by day | Dynamic: create day shards, assert viewer receives |
| P2.5 Time-sharded offline merge works | CRDT merge on same shard | Dynamic: concurrent writes to same day shard, assert merge |
| P2.6 All 5 shard resolutions compile | `.osv` shard by minute/hour/day/week/month | Static: compile each, assert schema |
| P2.7 Per-user layer isolation (audience scoping) | `{aud}` in path pattern | Static: expand pattern, assert isolation |
| P2.8 Sharding bounds memory working set | Selective subscription | Dynamic: create N shards, subscribe to 1, measure memory |

### WP3: Protocol Mechanics

**Model**: The sync protocol converges. Every message type has defined
serialization. The handshake authenticates peers. The dual-plane separates
durable and ephemeral data.

| Property | Declaration | Proof method |
|----------|------------|-------------|
| P3.1 Handshake authenticates owner | Hello, Welcome, PermitGrant, Ack | Dynamic: Tracer assert_sequence |
| P3.2 Invalid permit rejected | Permit validation | Dynamic: garbage permit, connection fails |
| P3.3 All messages serialize/deserialize | Tagged wire format | Static: proptest roundtrips |
| P3.4 3-step sync converges | SyncOffer, SyncAccept, SyncAck | Dynamic: offline write, reconnect, assert convergence |
| P3.5 Late joiner receives full state | Catch-up sync | Dynamic: write before viewer connects, viewer has data |
| P3.6 Bidirectional viewer sync | Viewer write path | Dynamic: viewer writes, owner receives |
| P3.7 Ephemeral datagrams deliver | Datagram channel | Dynamic: send ephemeral, receiver gets it |
| P3.8 Sync metadata layer discovery | `__sync_meta:{did}` | Static: create/read/mark_synced lifecycle |
| P3.9 State vectors track causal frontier | Version vector comparison | Dynamic: N syncs, assert vectors match |

### WP4: Capability Security and Governance

**Model**: The `.osv` declaration is the complete authority specification. The
permit system enforces it exactly. Every role gets exactly the access declared.

| Property | Declaration | Proof method |
|----------|------------|-------------|
| P4.1 Owner can write all declared layers | `allow owner to read, write, sync` | Static: permit roundtrip |
| P4.2 AI agent can write channels only | `allow ai_agent to read, write, sync on channel_messages` | Static: permit roundtrip |
| P4.3 AI agent cannot write presence | `allow ai_agent to read, sync on presence` (no write) | Static: permit roundtrip |
| P4.4 AI agent cannot grant DM access | No `grant, revoke` for ai_agent | Static: permit roundtrip |
| P4.5 Deny overrides allow | Policy engine invariant | Static: deny + allow, deny wins |
| P4.6 Delegation cannot escalate | no_escalation policy | Static: delegate write when only have read, denied |
| P4.7 DM isolation between viewers | `grant explicit` + `namespace creator` | Dynamic: viewer1 cannot see viewer0 DM |
| P4.8 Dynamic layer permit scoping | Schema pattern matching | Static: dynamic layer denied without layer permit |
| P4.9 Consent is required for reverse sync | sync_layer_consent token | Static: consent token chain verification |
| P4.10 Authorized peers list enforcement | authorized_peers fact | Static: in-list allowed, not-in-list denied |
| P4.11 Pattern matching is correct (property-based) | Wildcard expansion | Static: proptest, reflexive, segment-count, position-independent |
| P4.12 Delegation chain preserves scoping | owner, node, viewer | Static: full chain, viewer has correct subset |

### WP5: Runtime Surface and Builder Ergonomics

**Model**: `.osv` is a complete app specification language. Every grammar
construct compiles to correct runtime behavior. The compiler rejects invalid
specifications with precise error codes.

| Property | Declaration | Proof method |
|----------|------------|-------------|
| P5.1 Full `.osv` pipeline compiles | All grammar constructs | Static: parse, semantic, lower, assert output |
| P5.2 All layer kinds compile | map, list, text, counter, blob | Static: compile each kind |
| P5.3 All grant modes compile | open, explicit, role_scoped | Static: compile + assert lowered grant |
| P5.4 All cache policies compile | ui_window, memory_lru, sync_mode, retention_days | Static: compile each |
| P5.5 Role inheritance works | `role x inherits y` | Static: child inherits parent capabilities |
| P5.6 All lexer errors detected | E1001-E1005 | Static: 5 error inputs, 5 correct error codes |
| P5.7 All parser errors detected | E1101-E1117 | Static: 17 error inputs, 17 correct error codes |
| P5.8 All semantic errors detected | E2001-E2402 | Static: 10 error inputs, 10 correct error codes |
| P5.9 All sample apps compile | group-chat, my-shop, my-booking, osvauld-demos | Static: compile each, assert no errors |
| P5.10 Derivation pipeline auto-triggers | derive declaration | Dynamic: write source, node fires derivation, derived layer populated |
| P5.11 Lua validation enforces rules | validate_ops function | Static: allowed ops pass, denied ops fail |
| P5.12 Scribe binding manager works | scribe:bind pattern matching | Static: exact, wildcard, rebind lifecycle |

### WP6: Observability and Performance

**Model**: Protocol behavior is capturable and measurable. Benchmarks are
deterministic and reproducible.

| Property | Declaration | Proof method |
|----------|------------|-------------|
| P6.1 JSONL capture produces valid events | capture_start/capture_end | Dynamic: run scenario, capture, assert event types |
| P6.2 Capture timeline is merge-sortable | Multi-peer capture merge | Dynamic: capture on 3 peers, merge, assert sorted |
| P6.3 Sync throughput is measurable | chat_throughput benchmark | Bench: N messages, msgs/second |
| P6.4 Sync latency is measurable | sync_latency benchmark | Bench: write, convergence time |
| P6.5 Memory growth is bounded | multi_peer_convergence bench | Bench: N messages, RSS at checkpoints |
| P6.6 Perf regression is detectable | compare_perf.py | Bench: baseline vs current, BETTER/WORSE/SAME |

### WP7: Time Modeling and Long-Horizon Correctness

**Model**: Deterministic time control enables long-horizon verification without
wall-clock waiting.

| Property | Declaration | Proof method |
|----------|------------|-------------|
| P7.1 ManualClock advances deterministically | ClockSource trait | Static: advance, assert monotonic + unix |
| P7.2 Period formatting handles all resolutions | format_period() | Static: minute/hour/day/week/month with offsets |
| P7.3 Month offset crosses year boundary | Calendar arithmetic | Static: Jan-1=Dec, Jan+13=Feb next year |
| P7.4 Timer scheduler fires correctly | ManualClock + scheduler | Static: register, advance, assert fired |
| P7.5 Daily shards create at day boundaries | shard by day + ManualClock | Dynamic: advance 7 days, assert 7 shards |
| P7.6 Sharded offline merge across days | CRDT + time sharding | Dynamic: offline writes across days, merge |
| P7.7 `.osv` shard+retention semantics enforced | semantic validation | Static: shard without period, E2301 |
| P7.8 Year-long shard simulation | ManualClock advance 365 days | Dynamic: assert 365 daily shards, correct naming |

### WP8: AI as Protocol Participant

**Model**: AI is just a role. Its behavior is fully specified by its permit.
No special treatment, no grammar changes, no privileged bypass.

| Property | Declaration | Proof method |
|----------|------------|-------------|
| P8.1 AI agent is defined as a role | `role ai_agent;` in `.osv` | Static: compile, assert role in output |
| P8.2 AI permit has correct scoping | `allow ai_agent to ...` | Static: permit roundtrip, assert per layer |
| P8.3 AI can write to channels | channel_messages layer | Dynamic: AI writes, message syncs to other peers |
| P8.4 AI cannot write presence | presence layer (read-only) | Static + Dynamic: write attempt, denied |
| P8.5 AI cannot create DMs | dm_messages (no grant) | Static: grant action, denied |
| P8.6 AI has no UI entry point | No ui_app allow | Static: assert no ui_app access |
| P8.7 AI has no peer capabilities | No `can share, delegate, ...` | Static: assert empty capabilities |
| P8.8 AI actions appear in capture | JSONL attribution | Dynamic: AI writes, capture shows AI DID as sender |
| P8.9 AI token delegation chain is valid | owner, node, ai_agent | Static: chain has correct proof, audience, scoping |
| P8.10 AI cannot escalate via delegation | no_escalation | Static: AI delegates write on presence, denied |

---

## AI Agent Role Design

### Definition

AI agent is just a role. No grammar changes, no special syntax:

```osv
role ai_agent;
```

No peer capabilities (no share, delegate, relay, accept_publish, manage_access).

### Layer Permissions for Group Chat

| Layer | AI access | Rationale |
|-------|----------|-----------|
| `channel_messages` | read, write, sync | Can post messages to channels |
| `channel_messages_sharded` | read, write, sync | Same for time-sharded channels |
| `presence` | read, sync | Can see who is online, but is not "present" |
| `dm_messages` | read, sync | Can read DMs it is granted access to, cannot initiate |
| `app_data` | read, sync | Read-only on app configuration |

No grant/revoke on any layer. No ui_app access.

### What This Proves

1. AI requires zero special treatment — same role/permit/delegation machinery
2. AI scoping is declarative — change the `.osv`, get different access
3. AI actions are auditable — same capture/tracing infrastructure
4. AI cannot escalate — same no_escalation policy applies
5. AI is offline-capable — permits are cached locally, no online lookups

### Synthetic Spec: `specs/wp8_ai_participant.osv`

```osv
app "wp8-ai-participant" version "1.0.0" {
  role owner can share, delegate, manage_access;
  role node can relay, share, manage_access;
  role human_user;
  role ai_agent;

  layer public_channel as map {
    path "channels/{id}/messages";
    namespace shared;
    allow owner, node, human_user, ai_agent to read, write, sync;
  }

  layer presence as map {
    path "presence";
    namespace shared;
    allow owner, node, human_user to read, write, sync;
    allow ai_agent to read, sync;
  }

  layer private_dm as map {
    path "dms/{id}/messages";
    namespace creator;
    grant explicit;
    allow owner, node, human_user to read, write, sync;
    allow owner to grant, revoke;
    allow ai_agent to read, sync;
  }

  layer config as map {
    path "app:Config";
    namespace shared;
    allow owner to read, write, sync;
    allow node, human_user, ai_agent to read, sync;
  }

  ui_app "Chat" {
    entry "chat/app.lua";
    allow owner, human_user;
  }
}
```

Expected verification output:

```
== WP8: AI as Protocol Participant ==

Spec: specs/wp8_ai_participant.osv
Pipeline: compile -> sign -> delegate(owner->node->ai_agent) -> parse -> decide

Property 8.2: AI permit has correct scoping
  ai_agent.can_access("channels/*/messages", "read")   = true    PASS
  ai_agent.can_access("channels/*/messages", "write")  = true    PASS
  ai_agent.can_access("channels/*/messages", "sync")   = true    PASS
  ai_agent.can_access("presence", "read")              = true    PASS
  ai_agent.can_access("presence", "write")             = false   PASS
  ai_agent.can_access("presence", "sync")              = true    PASS
  ai_agent.can_access("dms/*/messages", "read")        = true    PASS
  ai_agent.can_access("dms/*/messages", "write")       = false   PASS
  ai_agent.can_access("dms/*/messages", "grant")       = false   PASS
  ai_agent.can_access("app:Config", "write")           = false   PASS
  ai_agent.has_ui_app_access("Chat")                   = false   PASS
  ai_agent.peer_capabilities                           = []      PASS

  Result: PASS (12/12 assertions)

Property 8.2 (comparison): human_user permit
  human_user.can_access("channels/*/messages", "write") = true   PASS
  human_user.can_access("presence", "write")            = true   PASS
  human_user.can_access("dms/*/messages", "write")      = true   PASS
  human_user.has_ui_app_access("Chat")                  = true   PASS

  Result: PASS (4/4 — human_user has strictly more access than ai_agent)
```

---

## Grammar Correctness as Formal Verification

### Principle

The `.osv` grammar is the formal specification language. Grammar correctness
means: the compiler accepts valid specs, rejects invalid specs with precise
error codes, and the compiled output produces correct permit decisions.

### Three Levels of Grammar Verification

**Level 1: Syntax** — the compiler accepts/rejects the right inputs

Every error code (E1001-E1194, E2001-E2402) has a dedicated test case.
Input: a minimal `.osv` snippet triggering exactly that error.
Expected: the specific error code.
Proves: the compiler detects this class of invalid input.

**Level 2: Compilation** — valid specs lower to correct PolicyFacts

For each grammar construct:
Input: a valid `.osv` spec exercising that construct.
Expected: CompiledArtifacts contain correct roles, schemas, policies.
Proves: lowering is faithful to declared intent.

**Level 3: Permit round-trip** — compiled specs produce correct access decisions

For each sample app and synthetic spec:
Input: `.osv` source.
Pipeline: compile, sign owner token, delegate to each role, parse, decide.
Expected: per-role, per-layer access decisions match `.osv` declarations.
Proves: the full pipeline (grammar, compiler, crypto, authorization) is correct.

### Error Code Coverage

| Category | Codes | Count | Target |
|----------|-------|-------|--------|
| Lexer errors | E1001-E1005 | 5 | 100% |
| Parser errors | E1101-E1117 | 17 | 100% |
| Semantic errors | E2001-E2402 | 10 | 100% |
| **Total** | | **32** | **100%** |

### Construct Coverage

| Construct | Variants | Currently tested | Target |
|-----------|---------|-----------------|--------|
| Layer kind | map, list, text, counter, blob | map, list | All 5 |
| Grant mode | open, explicit, role_scoped | open, explicit | All 3 |
| Shard resolution | minute, hour, day, week, month | day | All 5 |
| Cache policy | ui_window, memory_lru, sync_mode, retention_days | retention_days | All 4 |
| Namespace | shared, creator | shared | Both |
| Scope expression | All, LayerRef | All only | Both |
| Role inheritance | valid chain | cycle detection only | Both valid + cycle |
| Comments | `// ...` | untested | Tested |
| String escapes | `\"` | untested | Tested |

---

## Benchmark Methodology

All benchmarks use MockConnection (zero network latency) and ManualClock
(deterministic time). This isolates pure system cost from network/OS variance.

### Output Format

```json
{
  "benchmark": "chat_throughput",
  "params": { "peers": 2, "messages": 1000, "transport": "mock" },
  "results": {
    "send_throughput_msg_per_sec": 12400,
    "sync_convergence_ms": 847,
    "peak_rss_owner_mb": 18.2,
    "peak_rss_node_mb": 22.1,
    "peak_rss_viewer_mb": 16.8
  },
  "timestamp": "2026-02-21T00:00:00Z",
  "git_sha": "abc123"
}
```

Longitudinal comparison via existing `scripts/compare_perf.py`.

---

## DST Infrastructure

### What Exists

| Component | Status |
|-----------|--------|
| ManualClock (wall + monotonic) | Complete, wired into both binaries |
| MockConnection (in-memory transport) | Complete, instant delivery |
| Scenario builder (multi-peer) | Complete, connect/disconnect/publish |
| Tracer (message capture) | Complete, subsequence + count assertions |
| MockBlobStore (shared blob network) | Complete |
| mock_node_id (deterministic IDs) | Complete, blake3-derived |

### What is Missing

| Component | Status | Priority |
|-----------|--------|----------|
| tokio::time virtual clock | Missing | Defer — ManualClock covers app-level time |
| Network fault injection | Missing | Build when DST depth increases |
| Message interception layer | Missing | Build when DST depth increases |
| Seed-based test reproducibility | Missing | Add with FaultyConnection |
| Deterministic actor scheduling | Missing | Requires ractor changes |

### Approach

Start with what exists. ManualClock + MockConnection + deterministic identity
generation already enable reproducible scenarios. Build fault injection
(FaultyConnection) when the papers require adversarial properties.

---

## External Spec Citations

Each paper includes formal references where applicable.

| Technology | Citation |
|-----------|---------|
| UCAN | UCAN Spec v0.10.0 — https://github.com/ucan-wg/spec |
| QUIC | RFC 9000 — QUIC: A UDP-Based Multiplexed and Secure Transport |
| QUIC-TLS | RFC 9001 — Using TLS to Secure QUIC |
| Ed25519 | RFC 8032 — Edwards-Curve Digital Signature Algorithm |
| X25519 | RFC 7748 — Elliptic Curves for Security |
| AES-256-GCM | NIST SP 800-38D — Recommendation for GCM Mode |
| HKDF | RFC 5869 — HMAC-based Extract-and-Expand Key Derivation Function |
| ChaCha20Poly1305 | RFC 8439 — ChaCha20 and Poly1305 for IETF Protocols |
| BLAKE3 | BLAKE3 Spec — https://github.com/BLAKE3-team/BLAKE3-specs |
| Argon2 | RFC 9106 — Argon2 Memory-Hard Function |
| DID | W3C DID Core v1.0 — https://www.w3.org/TR/did-core/ |
| did:key | did:key Method — https://w3c-ccg.github.io/did-method-key/ |
| CRDTs | Shapiro et al., "A comprehensive study of CRDTs" (2011), INRIA RR-7506 |
| Loro | Loro Documentation — https://loro.dev/docs |
| BIP39 | BIP-0039 — https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki |
| iroh | iroh Documentation — https://iroh.computer/docs |

---

## Execution Phases

| Phase | Scope | Depends on | Deliverables |
|-------|-------|-----------|-------------|
| 1 | Extract `test_harness` from `integration_tests` | — | test_harness crate, integration_tests refactored |
| 2 | Create `osvauld_verify` binary skeleton | Phase 1 | Binary with CLI, report output |
| 3 | Write synthetic `.osv` specs | — | 10 specs in osvauld_verify/specs/ |
| 4 | Implement static verification | Phase 2+3 | Permit roundtrip + grammar coverage |
| 5 | Implement dynamic verification | Phase 1+2 | Protocol scenarios using test_harness |
| 6 | Implement benchmarks | Phase 1+2 | Throughput/latency/memory measurement |
| 7 | Add AI agent role to group-chat app.osv | Phase 3 | Modified sample app |
| 8 | Implement VerifyHandler + serve mode | Phase 2 | Python-compatible JSON-RPC backend |
| 9 | Run full verification, collect reports | Phase 4+5+6 | Verification transcripts |
| 10 | Write 8 papers around verification output | Phase 9 | 8 markdown files |

### Test Count Trajectory

| Category | Before | After |
|----------|--------|-------|
| Unit tests | ~160 | ~195 |
| Integration tests | 31 | ~40 |
| E2E tests | 11 | ~13 |
| Grammar tests | 7 | ~49 |
| Static verification properties | 0 | ~60 |
| Dynamic verification properties | 0 | ~25 |
| Benchmarks | 0 | ~8 |
| **Total** | **~209** | **~390** |

---

## Open Work (Deferred)

- FaultyConnection (latency/drop/reorder injection) — build when DST depth increases
- JS/platform comparison sections — deferred
- Formal threat model verification — requires adversarial test paths
- Hot/Cold sync tier enforcement verification — declared but possibly not fully enforced
- Revocation/rotation semantics — deferred until chain invalidation paths are measured
- Process-level sandbox guarantees — Lua confinement is runtime-level, not OS-level
- tokio::time::pause() integration — defer, ManualClock covers app-level time
