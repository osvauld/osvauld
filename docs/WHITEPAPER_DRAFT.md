# Osvauld Whitepaper (Draft v1)

## Coordinated Decentralization for Offline-First Collaborative Systems

**Subtitle:** A capability-secure protocol/runtime for sovereign applications built on CRDTs, UCAN, and QUIC.

---

## Abstract

Most collaborative systems still depend on centralized coordination services, server-local authorization policy, and always-online operation. Osvauld proposes an alternative: **coordination without platform centralization**. State is committed locally first, replicated asynchronously over peer links, and governed through signed delegatable capabilities rather than server-owned ACL state.

Osvauld combines five architectural primitives:

1. **Offline-first CRDT state** for convergent collaboration.
2. **Capability-based permits (UCAN)** for programmable authority and delegation.
3. **Encrypted transport and storage** for confidentiality and integrity within the permit-governed trust boundary.
4. **Sovereign coordination nodes** that can run on commodity hardware.
5. **A compact application surface** (Lua + Slint/Raylib) where developers primarily encode domain logic.

The result is a system where collaborative applications can be built without bespoke backend sync infrastructure, while protocol behavior remains inspectable through explicit message/state transitions and deterministic test controls.

---

## 1. Problem Statement

Contemporary collaborative application stacks typically embed three assumptions that are invalid in constrained, adversarial, or sovereign environments:

- Persistent internet connectivity.
- Trusted central infrastructure for identity and authorization.
- Request-response consistency models for collaborative state.

These assumptions produce structural fragility: degraded connectivity disables core UX, central outages collapse governance and access, and scaling collaboration shifts conflict semantics into ad hoc application code.

Osvauld is based on three alternate premises:

- Collaboration should continue offline.
- Authority should be portable and delegatable.
- Convergence should be data-structural, not request-transactional.

---

## 2. Design Goals and Non-Goals

The protocol/runtime is designed around the following goals:

1. **Offline-first correctness**: all user operations commit locally before network synchronization.
2. **Convergent replication**: eventually consistent multi-peer state via CRDT merge semantics.
3. **Capability security**: explicit, signed, delegatable permits for every authority boundary.
4. **Sovereign operations**: node coordination runnable on laptop, desktop, Raspberry Pi, or VPS.
5. **Low-friction app construction**: builders write Lua/UI logic, not custom sync and authorization servers.
6. **Protocol observability**: message traces and sync events are first-class artifacts.
7. **Long-horizon confidence**: deterministic time controls and offline simulation for deep scenario testing.

Explicit non-goals for v1 include globally linearizable transactions, mandatory always-online orchestration, and centralized policy lookups during normal operation.

---

## 3. System and Threat Model

### 3.1 Roles

Osvauld operates with three core roles:

- **Owner**: creates and governs spaces/pages.
- **Node (Kunki)**: coordinates sync, relays data, stores state, issues delegated layer permits.
- **Viewer**: receives delegated access and participates under permit constraints.

The node coordinates replication, but this is not equivalent to platform centralization: the coordinator is sovereign infrastructure, not a mandatory shared cloud authority.

### 3.2 Trust and Authority

- Identity and crypto keys are local.
- Authorization is expressed as UCAN permits.
- Delegation is explicit through permit templates.
- Sync acceptance and broadcast are constrained by per-layer permissions and consent.

### 3.3 Failure and Adversary Assumptions

- Peers may disconnect arbitrarily.
- Network partitions are expected, not exceptional.
- Reconnection is normal; sync is eventually convergent.
- Peers may be malicious or misconfigured; all authority-bearing operations require permit validation.

---

## 4. Architecture and Reference Implementations

Osvauld is a multi-crate runtime with strict boundaries:

- **herald**: identity, signing, encryption primitives.
- **transport**: QUIC streams/datagrams/blob transfer (message-agnostic byte pipe).
- **gurkha**: permit parsing and authorization decision logic.
- **scribe**: CRDT document actor and per-layer sync/authorization routing.
- **butler**: storage and service facade.
- **courier**: peer lifecycle, handshake, sync protocol orchestration.
- **lua_runtime**: app execution VM and bindings.
- **renderer_slint / renderer_raylib**: UI/game rendering surfaces.
- **sthalam / kunki**: user shell and sovereign node binaries.

This decomposition isolates authorization policy from transport mechanics, transport mechanics from app logic, and storage concerns from protocol routing.

### 4.1 Sthalam as a Browser-Class Protocol Client

`sthalam` should be understood as a browser-class client for the Osvauld protocol: analogous to the role a browser plays in the HTTP world.

In this framing, Osvauld is the protocol/runtime substrate, and `sthalam` is the flagship general-purpose client implementation.

`sthalam` combines:

- identity and key management,
- permit-aware app execution,
- CRDT-backed local-first state,
- peer connectivity and sync orchestration,
- observability and control surfaces.

`sthalam` is therefore the primary user-facing reference implementation. The distinction remains explicit: protocol capability is not owned by any single client.

### 4.2 Protocol Scope Beyond Desktop Collaboration

Osvauld is not limited to desktop collaboration UX. The same protocol and capability model can support:

- IoT coordination and smart-home automation,
- live telemetry and machine data streams,
- rich media workflows,
- specialized vertical clients beyond the desktop shell.

The transport substrate already exposes stream/datagram channels and blob transfer primitives. This yields a direct path toward audio/video and media-heavy workloads as client render/runtime layers mature.

---

## 5. Data Model and Local-First Semantics

### 5.1 Hierarchy

State is organized as:

**Identity -> Space -> Page -> Layer**

Layers are CRDT-backed containers (map/list/text) with role-aware read/write/sync semantics.

### 5.2 Local-First Write Path

Every mutation follows the same ordering invariant:

1. Mutate local CRDT layer.
2. Commit locally.
3. Emit updates for async sync.
4. Converge with peers when connectivity permits.

No network round-trip is required for local progress.

### 5.3 Discovery and Dynamic Topology

Layer discovery uses `__sync_meta:{did}` CRDT layers. Discovery metadata is itself convergent state, allowing peers and node to independently write/merge layer availability and sync readiness.

---

## 6. Protocol Mechanics

### 6.1 Transport

Courier protocol runs over QUIC:

- Streams for ordered protocol messaging.
- Datagrams for fire-and-forget low-latency ephemerals.

### 6.2 Handshake and Authentication

Handshake follows explicit message exchanges (Hello/Welcome/PermitGrant/Ack with reconnection variants) and validates permit context, audience, and role expectations prior to authenticated state transitions.

### 6.3 Layer Sync

Durable sync uses a 3-step protocol:

1. **SyncOffer** (state + vector)
2. **SyncAccept** (receiver vector)
3. **SyncAck** (completion or resync loop)

State vectors encode causal frontier information and reduce redundant transfer.

### 6.4 Pull-Based Layer Subscription

Peers discover layers through sync metadata, then request them explicitly (`LayerSubscribe` / `LayerSubscribeAck`) with consent and bundled permit/data initialization.

---

## 7. Capability Security and Governance

Osvauld uses UCAN permits as the authority substrate.

### 7.1 Why Capabilities Instead of Central ACL Tables

- Permits are signed, portable authority objects.
- Delegation rules are embedded and recursive (`issue_on`).
- Authorization can be derived from signed token facts, not hidden mutable server tables.

### 7.2 Permit Scope

Permits define, per role and context:

- Operations allowed.
- Layer read/write/sync permissions.
- Layer patterns and dynamic schemas.
- Delegation capabilities.
- Ephemeral function allowances.

### 7.3 Consent as a First-Class Primitive

Sync consent permits define what data flow a peer explicitly allows in the reverse direction. This supports bidirectional authorization rather than unilateral push assumptions.

### 7.4 Dynamic Layer Authority

Dynamic layers are created at runtime via schema patterns; node-side matching and permit issuance determine distribution to eligible peers. This provides scalable, capability-safe growth of collaboration topology.

---

## 8. Dual-Plane Collaboration: Durable CRDT and Ephemeral Realtime

Osvauld deliberately separates two data planes:

1. **Durable Plane (CRDT layers)**: authoritative, persistent, convergent state.
2. **Ephemeral Plane (datagrams)**: low-latency, non-persistent signals.

This split avoids forcing high-frequency signals into durable logs while preserving correctness for shared state.

### 8.1 Ephemeral Use Cases

- Typing indicators.
- Presence and cursors.
- Multiplayer game position/bullet/enemy telemetry.
- Lightweight diagnostics signaling.

### 8.2 Systems Rationale

Systems that use only durable replication for realtime interactions often overpay in storage churn and merge overhead. Systems that use only ephemeral channels lose recoverability and eventual state guarantees. Osvauld uses both planes, each for the right class of signal.

---

## 9. Time as a Protocol Dimension

A major practical requirement for long-lived collaborative systems is confidence over long horizons (weeks, months, years), not just short interactive sessions.

Osvauld test infrastructure includes deterministic runtime time control in test mode:

- Set explicit runtime time.
- Advance time deterministically.
- Simulate offline windows and reconnection.

This allows long-horizon simulation (for example day-sharding, retention windows, and periodic workflows) without waiting for wall-clock time.

---

## 10. Layer-Native Sharding and Scale-Out Topology

Osvauld scales data topology through layer composition rather than monolithic document growth.

### 10.1 Trivial Sharding

Sharding is a naming and schema decision:

- Per-user layers: `{page_id}/orders/{aud}`
- Per-channel layers: `channels/{id}/messages`
- Per-period layers: `messages/{YYYY-MM-DD}`

For time-series collaboration data (for example chat), period sharding can be selected at the required operational resolution:

- Daily resolution: one year of data maps to 365 logical shards/layers.
- Minute resolution: one day maps to 1,440 logical shards/layers when fine-grained partitioning is required.

This keeps write/read working sets bounded while preserving a deterministic path model for retention, replay, and selective sync.

Creating new shards is equivalent to creating new layers under permit-validated patterns and schema constraints.

### 10.2 Benefits

- Reduced contention domains.
- Targeted permissioning.
- Efficient selective sync.
- Natural archival and lifecycle policies.

### 10.3 Dynamic Growth

Layer creation and discovery are protocol-native and do not require schema migrations in central SQL backends.

---

## 11. Runtime Surface and Builder Ergonomics

Osvauld's adoption properties depend on builder velocity.

### 11.1 Compact Application Surface

Builders primarily use:

- **Lua** for business logic.
- **Slint or Raylib** for UI/rendering.
- **scribe API** for layers, binding, and ephemerals.
- **permit API** for capability-aware checks.

### 11.2 Declarative Binding and Incremental UI Updates

`scribe:bind(...)` provides automatic layer-to-UI synchronization with surgical updates (`key`, `max_items`, wildcard binding, rebind support), reducing manual refresh and state plumbing code.

### 11.3 Reference Application Set

Sample apps include:

- Group chat (typing/presence/read tracking).
- Multiplayer tank game (node loop + ephemeral realtime plane).
- Snake and math simulations.
- E-commerce and booking flows with role-aware permits.
- Collaborative canvas and photo gallery.

This demonstrates support for both line-of-business and interactive realtime workloads within the same protocol/runtime model.

### 11.4 Why Building Apps in Sthalam Is Trivial

In Osvauld, `sthalam` plays the browser-class role: it supplies identity, sync, permits, rendering, and runtime wiring so application authors mostly write domain behavior.

The complexity removed from app code includes:

- peer networking and reconnect logic,
- CRDT merge and state-vector sync semantics,
- permit parsing and authorization decisions,
- layer discovery/replay and subscription lifecycle,
- cross-peer event routing and diagnostics plumbing.

Application code therefore reduces to a compact pattern:

1. Declare page roles/layers/apps in `app.osv`.
2. Bind UI properties to layers with `scribe:bind(...)`.
3. Mutate layers and send ephemerals from Lua handlers.
4. Let protocol/runtime machinery handle replication, convergence, and policy enforcement.

This is the sense in which app building is "trivial": not because the protocol is simple, but because protocol complexity is already packaged into the runtime.

### 11.5 Multi-App Pages and Shared Data Interfaces

A page is a shared data boundary that may host multiple apps. Distinct apps on the same page can read/write the same underlying layers and present different interaction models over identical state.

Examples of what this unlocks:

- one app optimized for operators, another for end users, over the same data,
- a control app and an analytics app backed by shared layers,
- a realtime view (ephemeral-heavy) and an audit/workflow view (durable-heavy),
- progressive specialization without schema migration or duplicate backends.

Because data is layer-addressed and permit-scoped, interoperability between sibling apps is a default property of the model, not an afterthought integration task.

### 11.6 Lua Runtime Confinement Model

Application logic executes inside the Lua runtime with platform-provided bindings (`scribe`, `permit`, `ui`, `api`, `timer`, `page`, `derivation`) rather than unrestricted direct access to protocol internals.

Practically, this gives a confinement model where:

- authority is mediated through permit checks,
- data access is layer-scoped,
- node-only behavior is separated from UI paths,
- and app behavior is observable through the same protocol/event capture surfaces.

This should be read as capability-oriented runtime confinement, not as a claim of opaque end-to-end secrecy against the coordinating node.

At present, this is best described as an application/runtime-level confinement model, not a full operating-system sandbox guarantee. Hard isolation primitives (for example process-level sandboxing policies) can be layered as deployment hardening where required.

### 11.7 Renderer Surfaces: Slint and Raylib

Osvauld supports two renderer classes for different workload profiles:

- **Slint** for declarative application interfaces and model-driven UI,
- **Raylib** for immediate-mode interactive graphics and game-style loops.

This dual-renderer model enables a single protocol/runtime to serve both business UI and realtime simulation/game workloads without changing the data/auth/sync substrate.

### 11.8 Efficiency Controls at the App Layer

Efficiency is exposed as runtime-level controls, not left entirely to app-specific optimization:

- declarative surgical updates via `scribe:bind(...)` with stable keys,
- bounded UI models via `max_items`,
- dynamic rebinding without full state rebuild patterns,
- separation of durable CRDT state from high-frequency ephemeral signals.

These controls reduce unnecessary UI churn, network over-send, and memory growth pressure in long-running collaborative sessions.

---

## 12. Observability and Diagnostics by Design

Protocol systems fail silently when observability is bolted on late. Osvauld includes structured event capture by default.

Captured JSONL streams can include:

- Protocol message traces.
- Courier business events.
- Scribe page updates.
- Sync events and layer authorization transitions.
- Optional tracing logs.

Capture can be merged across node and peers into timeline-sorted artifacts, enabling reproducible debugging and machine-assisted analysis.

### 12.1 Diagnostic Workflow

Diagnostics is treated as an operational primitive, not a post-hoc debugging add-on:

1. Start capture on node and peers.
2. Execute scenario (online/offline transitions, layer creation, sync, permissions).
3. End capture and merge into a single time-sorted JSONL artifact.
4. Inspect protocol/event timeline or feed artifact into analysis tooling.

This workflow makes failure localization explicit (message flow, authorization transition, or application-layer behavior) and supports repeatable regression debugging.

### 12.2 What Gets Proven by Capture

Event capture enables concrete verification of claims such as:

- a dynamic layer was created and announced,
- permit issuance happened before downstream sync,
- sync offer/accept/ack sequences completed as expected,
- authorization moved from pending to authorized,
- ephemeral and durable paths behaved as intended under reconnect.

---

## 13. Security Model

### 13.0 Trust Boundary Clarification

Osvauld is not modeled as opaque end-to-end ciphertext between application peers and node. The coordination node is an authorized protocol participant that may decrypt and interpret permitted state in order to validate, issue delegation, coordinate sync, and serve downstream viewers.

Accordingly, security claims in this document are scoped as:

- encryption in transit and at rest,
- explicit capability-gated access,
- and auditable protocol behavior at each authority boundary.

### 13.1 Cryptographic Boundaries

- Identity keys for signing.
- Encryption keys and session derivation for peer channels.
- Signed permit chain verification before authority use.

### 13.2 Authorization Boundaries

- Per-layer read/write/sync checks.
- Dynamic schema matching for runtime-created layers.
- Consent gates for reverse sync permissions.

### 13.3 Runtime Boundaries

- Node and user modes have explicit guarded behavior.
- Crate-level boundaries prevent policy leakage through implicit dependencies.

---

## 14. Limitations and Open Work

This draft intentionally distinguishes architecture direction from implementation maturity. Current open areas include:

- Additional hardening on some handshake signature paths.
- Ongoing reliability polish in advanced multi-viewer and asset scenarios.
- Continued optimization for actor overhead and permit issuance bursts.
- UX tooling around conflict visibility and operator workflows.

These constraints do not invalidate the architecture; they should remain explicit in protocol-facing documentation.

---

## 15. AI-Era Relevance (Non-Normative)

Although AI interface conventions are out of scope for this draft's normative sections, Osvauld's architecture aligns with AI-native system requirements:

- explicit capability boundaries,
- deterministic observability artifacts,
- programmable local runtime surfaces,
- and low-friction app extension.

An important consequence is that AI can be modeled as another protocol participant (a peer/user with explicit permits), rather than as a privileged backend operator. This keeps AI actions within the same authority, sync, and audit boundaries as human participants.

---

## 16. Conclusion

Osvauld presents a practical alternative to centralized collaboration stacks: a coordinated-decentralized protocol/runtime where offline-first CRDT state, capability security, and sovereign infrastructure are first principles.

Its central claim is that decentralized collaboration can be both builder-accessible and protocol-rigorous in the same system.

---

## Appendix A: Performance and Resource Notes

Detailed benchmark data, heap/flame analysis, and tooling workflow are maintained separately in `docs/PERFORMANCE_TESTING.md`.

Highlights from the current baseline direction:

- Strong throughput improvement post-refactor.
- Controlled memory growth rates under sync load.
- Predictable renderer-related RSS overhead vs allocator-tracked heap.
- Identified hotspots for ongoing optimization (permit issuance, actor dispatch, flush export costs).

### A.1 Performance Test Surface

Current performance tooling covers:

- end-to-end sync throughput under configurable message volumes,
- idle and peak memory profiling,
- heap attribution and leak-style analysis,
- flamegraph hotspot extraction,
- consolidated machine-readable benchmark reports for longitudinal comparison.

### A.2 Why This Matters for the Protocol

Performance and diagnostics are linked:

- performance tests validate operational viability,
- capture artifacts explain why a run behaved the way it did,
- together they turn protocol changes into measurable, auditable outcomes.

### A.3 Renderer/GPU Notes

Current shell memory analysis shows a meaningful RSS-to-heap gap attributable to GPU-mapped renderer memory (for example Slint/femtovg GL context, textures, and framebuffers). This is expected for GPU-accelerated UI paths and should be interpreted separately from allocator-tracked heap growth.

Operationally, this means:

- allocator-level heap can remain controlled while RSS includes renderer/GPU residency,
- renderer choice and UI update patterns are material to end-to-end efficiency,
- protocol optimizations and renderer optimizations should be measured as separate dimensions.

---

## Appendix B: Protocol and Message Index

Canonical message structs, tags, and wire encoding are in `courier/src/message.rs`.

Key protocol families:

- Handshake: `Hello`, `Welcome`, `PermitGrant`, `Ack`, `Rejected`
- Sync: `SyncOffer`, `SyncAccept`, `SyncAck`, `SyncReset`, `SyncSnapshot`
- Viewer onboarding: `SpaceRequest`, `SpaceData`, `SpaceDataAck`
- Consent: `SyncConsentGrant`, `SyncConsentAck`
- Dynamic layer pull: `LayerSubscribe`, `LayerSubscribeAck`
- Shareable links: `GetShareableLinkRequest`, `GetShareableLinkResponse`

---

## Appendix C: Source Map for This Draft

Primary references:

- `docs/ARCHITECTURE.md`
- `docs/PROTOCOL.md`
- `docs/DATA_MODEL.md`
- `docs/SCRIBE_INTERNALS.md`
- `docs/GURKHA_INTERNALS.md`
- `docs/OBSERVABILITY.md`
- `docs/PERFORMANCE_TESTING.md`
- `docs/app-dev/LUA_API.md`
- `docs/app-dev/SCRIBE_API.md`
- `docs/app-dev/PERMITS.md`
- `docs/app-dev/TESTING.md`
- `USER_NODE_CONNECTION_FLOW.md`
- `sample_apps/osvauld-demos/*`
