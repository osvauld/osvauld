use serde::{Deserialize, Serialize};

pub type RoleId = String;
pub type LayerId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSpec {
    pub name: String,
    pub version: String,
    pub roles: Vec<RoleDecl>,
    pub layers: Vec<LayerDecl>,
    pub derives: Vec<DeriveDecl>,
    pub ui_apps: Vec<UiAppDecl>,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerDecl {
    pub name: LayerId,
    pub kind: LayerKind,
    pub path: String,
    pub namespace: LayerNamespace,
    pub grant: GrantMode,
    pub time: TimeModel,
    pub cache: Vec<CachePolicy>,
    pub access: Vec<AccessRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Map,
    List,
    Text,
    Counter,
    Blob,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerNamespace {
    Shared,
    Creator,
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
pub struct AccessRule {
    pub roles: Vec<RoleId>,
    pub actions: Vec<Action>,
    pub scope: ScopeExpr,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopeExpr {
    All,
    LayerRef(LayerId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeriveDecl {
    pub target: LayerId,
    pub kind: LayerKind,
    pub source: LayerId,
    pub cache: Vec<CachePolicy>,
    pub using: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAppDecl {
    pub name: String,
    pub entry: String,
    pub allowed_roles: Vec<RoleId>,
}
