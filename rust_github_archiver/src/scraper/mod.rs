// GitHub Archive scraper module placeholder
// This will contain the main scraping logic, file processing, and data extraction

pub mod archive_scraper;
pub mod file_processor;
pub mod downloader;
pub mod state;
pub mod main_scraper;

pub use state::{ScraperManager, ScraperState, ScraperStatus};
pub use archive_scraper::{ArchiveScraper, ArchiveFile, ProcessingResult as ArchiveProcessingResult, ScrapingStats};
pub use file_processor::{FileProcessor, ProcessingResult as FileProcessingResult, GitHubEvent, EventBatch, RepositoryInfo, ActorInfo, ProcessingConfig};
pub use downloader::{Downloader, DownloadResult, DownloadStatus, DownloadConfig};
pub use main_scraper::{MainScraper, MainScraperStatus};
