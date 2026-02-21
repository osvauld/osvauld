//! Domains - Core domain models for Osvauld
//!
//! This crate contains pure domain models with minimal dependencies.
//! No business logic, just data structures and simple conversions.
//!
//! ## Models
//!
//! - **Layer**: CRDT wrapper around LoroDoc
//! - **Page**: App instance within a Space
//! - **Space**: Container for Pages
//! - **Node**: Sovereign nodes and owner info
//! - **Contact**: Known users/peers
//! - **Identity**: User identity and encrypted keys
//! - **Asset**: Encrypted static asset metadata
//! - **Device**: Device data
//! - **Query**: Query types for data access

pub mod asset;
pub mod clock;
pub mod contact;
pub mod device;
pub mod identity;
pub mod layer;
pub mod manifest;
pub mod node;
pub mod page;
pub mod query;
pub mod space;
pub mod sthithi;

// Re-export all public types
pub use asset::{AssetAck, AssetMetadata, AssetReady, AssetRequest};
pub use clock::*;
pub use contact::{ConnectionDeviceInfo, ContactData, ContactType, DeviceInfo};
pub use device::DeviceData;
pub use identity::*;
pub use layer::*;
pub use manifest::AppManifest;
pub use node::*;
pub use page::{DecryptedPage, Page, PageData, PageMeta, PreparedPage};
pub use query::*;
pub use space::*;
pub use sthithi::*;
