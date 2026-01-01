pub mod core;
pub mod plugins;
pub mod tui;

pub use plugins::traits::{ChapterInfo, MangaPlugin, SearchResult};
pub use core::downloader::Downloader;
pub use core::state::MangaState;

pub use plugins::asura::AsuraScans;
pub use plugins::demonic::DemonicScans;
pub use plugins::flame::FlameComics;