//! Butler API Facade
//!
//! Provides grouped sub-object APIs for cleaner organization:
//! - `butler.spaces()` - Space operations
//! - `butler.pages()` - Page operations
//! - `butler.apps()` - App layer operations
//! - `butler.files()` - File storage operations
//! - `butler.publish()` - Publish operations
//! - `butler.assets()` - Asset operations
//! - `butler.permits()` - Permit operations
//! - `butler.nodes()` - Node operations
//! - `butler.contacts()` - Contact operations
//!
//! ## Usage
//! ```ignore
//! // Old style (still works)
//! butler.create_space(name, owner, template).await?;
//!
//! // New style (preferred)
//! butler.spaces().create(name, owner, template).await?;
//! butler.apps().get_files(page_id, app_name).await?;
//! butler.files().save(page_id, path, content).await?;
//! butler.publish().prepare_page(page_id, node_pubkey, enc_key).await?;
//! butler.assets().upload(page_id, data, filename, mime).await?;
//! ```

mod apps;
mod assets;
mod contacts;
mod files;
mod nodes;
mod pages;
mod permits;
mod publish;
mod spaces;

pub use apps::AppsApi;
pub use assets::AssetsApi;
pub use contacts::ContactsApi;
pub use files::FilesApi;
pub use nodes::NodesApi;
pub use pages::PagesApi;
pub use permits::PermitsApi;
pub use publish::PublishApi;
pub use spaces::SpacesApi;
