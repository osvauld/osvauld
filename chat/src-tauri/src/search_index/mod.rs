mod extractor;
mod manager;
mod operations;
mod search_types;
mod storage;

pub use manager::SearchIndexManager;
pub use search_types::{IndexError, SearchResult, SearchResultType};

// Re-export commonly used functions
pub use manager::create_search_index;
