use serde::{Deserialize, Serialize};

pub type RoleId = String;
pub type LayerId = String;

// ─── Top Level ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSpec {
    pub name: String,
    pub version: String,
    pub roles: Vec<RoleDecl>,
    pub sthithis: Vec<SthithiDecl>,
    pub layers: Vec<LayerDecl>,
    pub derives: Vec<DeriveDecl>,
    pub relays: Vec<RelayDecl>,
    pub validates: Vec<ValidateDecl>,
    pub permits: Vec<PermitDecl>,
    pub ui_apps: Vec<UiAppDecl>,
}

// ─── Roles ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleDecl {
    pub name: RoleId,
    pub inherits: Option<RoleId>,
    pub capabilities: Vec<PeerCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerCapability {
    Share,
    Delegate,
    Relay,
    AcceptPublish,
    ManageAccess,
}

// ─── Entity Schema (Sthithi shape) ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SthithiDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub transitions: Vec<TransitionBlock>,
    pub entity_rules: Vec<EntityRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDecl {
    pub name: String,
    pub required: bool,
    pub immutable: bool,
    pub default: Option<Literal>,
    pub source: Option<FieldSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldSource {
    Clock,
    Peer,
    Mode,
    SelfRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Literal {
    String(String),
    Int(u32),
    Bool(bool),
    Null,
}

// ─── State Transitions ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionBlock {
    pub field: String,
    pub rules: Vec<TransitionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRule {
    pub from_state: String,
    pub to_state: String,
    pub allowed_roles: Vec<RoleId>,
    pub predicate: Option<PredicateExpr>,
}

// ─── Entity Rules ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityRule {
    OnUpdateSet {
        field: String,
        source: EntityActionSource,
    },
    OnDeleteReject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityActionSource {
    FromClock,
    FromPeer,
    FromMode,
    ToLiteral(Literal),
}

// ─── Predicate Expressions ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateExpr {
    pub predicates: Vec<Predicate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Predicate {
    Is {
        field: String,
        value: ValueRef,
    },
    IsNot {
        field: String,
        value: ValueRef,
    },
    In {
        field: String,
        values: Vec<ValueRef>,
    },
    Above {
        field: String,
        value: ValueRef,
    },
    Below {
        field: String,
        value: ValueRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueRef {
    SelfRef,
    Peer,
    User,
    Node,
    StringLit(String),
    IntLit(u32),
    BoolTrue,
    BoolFalse,
}

// ─── Layers ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerDecl {
    pub name: LayerId,
    pub kind: LayerKind,
    pub entity_binding: Option<String>,
    pub path: String,
    pub namespace: LayerNamespace,
    pub grant: GrantMode,
    pub time: TimeModel,
    pub cache: Vec<CachePolicy>,
    pub access: Vec<AccessRule>,
    pub dynamic_by: Option<Vec<String>>,
    pub create_allow: Option<Vec<RoleId>>,
    pub discover: Option<DiscoverMode>,
    pub retain: Option<u32>,
    pub broadcasts: Vec<BroadcastStmt>,
    pub order: Option<OrderStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Map,
    List,
    Text,
    Counter,
    Blob,
    Tree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerNamespace {
    Shared,
    Page,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrantMode {
    Open,
    Explicit,
    RoleScoped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeModel {
    Unsharded,
    TimeSharded {
        resolution: Resolution,
        period_var: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMode {
    FullSnapshot,
    Incremental,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachePolicy {
    UiWindow { max_items: u32 },
    MemoryLru { max_layers: u32 },
    SyncMode(SyncMode),
    RetentionDays(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoverMode {
    Sync,
    Grant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BroadcastStmt {
    pub target: BroadcastTarget,
    pub debounce: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BroadcastTarget {
    All,
    Granted,
    ToRoles(Vec<RoleId>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderStmt {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

// ─── Access Rules ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessRule {
    pub roles: Vec<RoleId>,
    pub actions: Vec<Action>,
    pub predicate: Option<PredicateExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Create,
    Read,
    Write,
    Sync,
    Grant,
    Revoke,
}

// ─── Derived Layers ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeriveDecl {
    pub target: LayerId,
    pub kind: LayerKind,
    pub source: LayerId,
    pub cache: Vec<CachePolicy>,
    pub using: Option<String>,
}

// ─── Permit Declarations ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermitDecl {
    pub name: String,
    pub roles: Vec<RoleId>,
    pub statements: Vec<PermitStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermitStmt {
    Allow(PermitAllow),
    Issue(PermitIssue),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermitAllow {
    pub actions: Vec<Action>,
    pub layer: String,
    pub predicate: Option<PredicateExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermitIssue {
    pub roles: Vec<RoleId>,
}

// ─── Relay Declarations ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayDecl {
    pub layer: String,
    pub event: RelayEvent,
    pub handler: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayEvent {
    Insert,
    Update,
    Delete,
}

// ─── Validate Declarations ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidateDecl {
    pub layer: String,
    pub handler: String,
}

// ─── UI App Declarations ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAppDecl {
    pub name: String,
    pub allowed_roles: Vec<RoleId>,
}

/// Sanitize a ui_app display name into a valid layer name / identifier.
///
/// Rules: lowercase, spaces/hyphens → underscores, strip non-alphanumeric.
/// Example: `"Group Chat"` → `"group_chat"`, `"Snake-Game"` → `"snake_game"`
pub fn sanitize_app_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == ' ' || c == '-' {
                '_'
            } else {
                '_'
            }
        })
        .collect::<String>()
        // Collapse multiple underscores
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
