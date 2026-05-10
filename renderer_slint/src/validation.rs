use std::path::{Path, PathBuf};

/// Validate all Slint files in a directory using the Slint compiler
///
/// **Context**: Called before importing pages/creating spaces to catch syntax errors early
pub async fn validate_slint_files(page_dir: &Path) -> Result<(), String> {
    use slint_interpreter::Compiler;
    use walkdir::WalkDir;

    let slint_files: Vec<PathBuf> = WalkDir::new(page_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().file_name().is_some_and(|name| name == "app.slint"))
        .map(|e| e.path().to_path_buf())
        .collect();

    if slint_files.is_empty() {
        return Ok(());
    }

    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;

        let mut errors = Vec::new();

        for slint_path in slint_files {
            tracing::info!("Validating Slint file: {}", slint_path.display());
            let mut compiler = Compiler::default();
            widgets::register_library(&mut compiler);
            let result = rt.block_on(compiler.build_from_path(&slint_path));

            let compile_errors: Vec<_> = result
                .diagnostics()
                .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
                .map(|d| d.to_string())
                .collect();

            if !compile_errors.is_empty() {
                errors.push(format!(
                    "{}:\n  {}",
                    slint_path.display(),
                    compile_errors.join("\n  ")
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("Slint compilation errors:\n{}", errors.join("\n")))
        }
    })
    .await
    .map_err(|e| format!("Validation task failed: {}", e))?
}
