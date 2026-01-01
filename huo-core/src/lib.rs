pub mod core;
pub mod plugins;
pub mod tui;

// Re-export commonly used types
pub use plugins::traits::{ChapterInfo, MangaPlugin, SearchResult};
pub use core::downloader::Downloader;
pub use core::state::MangaState;

// Re-export plugin implementations
pub use plugins::asura::AsuraScans;
pub use plugins::demonic::DemonicScans;
pub use plugins::flame::FlameComics;