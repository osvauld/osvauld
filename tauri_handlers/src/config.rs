/// Configuration for handlers, injected via Tauri's managed state
#[derive(Clone, Debug)]
pub struct HandlerConfig {
    pub domain: String,
}

impl HandlerConfig {
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
        }
    }
}
