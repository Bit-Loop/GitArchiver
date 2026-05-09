// GitHub Archive scraping module surface: download, process, and runtime control.

pub mod archive_scraper;
pub mod downloader;
pub mod file_processor;
pub mod main_scraper;
pub mod state;

pub use archive_scraper::{
    ArchiveFile, ArchiveScraper, ProcessingResult as ArchiveProcessingResult, ScrapingStats,
};
pub use downloader::{DownloadConfig, DownloadResult, DownloadStatus, Downloader};
pub use file_processor::{
    ActorInfo, EventBatch, FileProcessor, GitHubEvent, ProcessingConfig,
    ProcessingResult as FileProcessingResult, RepositoryInfo,
};
pub use main_scraper::{MainScraper, MainScraperStatus};
pub use state::{ScraperManager, ScraperState, ScraperStatus};
