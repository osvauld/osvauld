//! Osvauld native widgets exposed to Lua apps.
//!
//! Apps consume widgets as a Slint library:
//!
//! ```slint
//! import { RichTextEdit } from "@osvauld/widgets";
//! ```
//!
//! The renderer registers this library on the slint-interpreter `Compiler`
//! before parsing app sources via [`register_library`].
//!
//! ## Skeleton scope (milestone 1)
//!
//! Currently exposes one widget — [`RichTextEdit`] — implemented as a thin
//! Slint `TextInput` wrapper. This proves the import / ComponentFactory /
//! Lua-binding pipeline end-to-end. Milestone 2 replaces the inner body with
//! a parley-driven custom-rendered surface backed by `LoroText`.

use std::path::PathBuf;

pub mod richtext;

/// Path to the `widgets/` directory containing widget `.slint` sources.
///
/// `slint-interpreter`'s `Compiler::set_library_paths` resolves
/// `@osvauld/widgets` against this path so app source can write
/// `import { RichTextEdit } from "@osvauld/widgets";`.
pub fn library_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("widgets")
}

/// Register the `@osvauld/widgets` library on a slint-interpreter `Compiler`.
///
/// **Context**: called once during renderer setup before `Compiler::build_*`.
/// **We mutate**: the compiler's library path map.
pub fn register_library(compiler: &mut slint_interpreter::Compiler) {
    let mut paths = compiler.library_paths().clone();
    paths.insert("osvauld".to_string(), library_path());
    compiler.set_library_paths(paths);
}
