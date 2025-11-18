/// Configuration for handlers, injected via Tauri's managed state
/// Currently unused but kept for future configuration needs
#[derive(Clone, Debug, Default)]
pub struct HandlerConfig {}

impl HandlerConfig {
    pub fn new() -> Self {
        Self {}
    }
}
