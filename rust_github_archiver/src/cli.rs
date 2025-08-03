use std::env;
use std::process;
use clap::{Arg, Command, ArgMatches};
use anyhow::Result;
use tracing::{info, error};

use crate::core::Config;
use crate::scraper::MainScraper;
use crate::api::ApiServer;

pub struct CliApp {
    config: Config,
}

impl CliApp {
    pub fn new() -> Result<Self> {
        let config = Config::new(None)?;
        Ok(Self { config })
    }

    pub fn run() -> Result<()> {
        let matches = Self::build_cli().get_matches();
        
        let mut app = Self::new()?;
        
        match matches.subcommand() {
            Some(("server", sub_matches)) => {
                tokio::runtime::Runtime::new()?.block_on(app.run_server(sub_matches))
            }
            Some(("scraper", sub_matches)) => {
                tokio::runtime::Runtime::new()?.block_on(app.run_scraper(sub_matches))
            }
            Some(("process", sub_matches)) => {
                tokio::runtime::Runtime::new()?.block_on(app.process_file(sub_matches))
            }
            Some(("download", sub_matches)) => {
                tokio::runtime::Runtime::new()?.block_on(app.download_file(sub_matches))
            }
            Some(("status", _)) => {
                tokio::runtime::Runtime::new()?.block_on(app.show_status())
            }
            Some(("cleanup", _)) => {
                tokio::runtime::Runtime::new()?.block_on(app.cleanup())
            }
            _ => {
                // Default: run both server and scraper
                tokio::runtime::Runtime::new()?.block_on(app.run_full())
            }
        }
    }

    fn build_cli() -> Command {
        Command::new("github_archiver")
            .version("2.0.0")
            .author("GitHub Archiver Team")
            .about("Professional GitHub Archive Scraper in Rust")
            .subcommand(
                Command::new("server")
                    .about("Run the web API server")
                    .arg(
                        Arg::new("port")
                            .short('p')
                            .long("port")
                            .value_name("PORT")
                            .help("Port to run the server on")
                            .default_value("8081")
                    )
                    .arg(
                        Arg::new("host")
                            .short('h')
                            .long("host")
                            .value_name("HOST")
                            .help("Host to bind the server to")
                            .default_value("0.0.0.0")
                    )
            )
            .subcommand(
                Command::new("scraper")
                    .about("Run the archive scraper")
                    .arg(
                        Arg::new("continuous")
                            .short('c')
                            .long("continuous")
                            .help("Run in continuous mode")
                            .action(clap::ArgAction::SetTrue)
                    )
                    .arg(
                        Arg::new("max-files")
                            .short('m')
                            .long("max-files")
                            .value_name("COUNT")
                            .help("Maximum number of files to process")
                    )
            )
            .subcommand(
                Command::new("process")
                    .about("Process a specific archive file")
                    .arg(
                        Arg::new("file")
                            .short('f')
                            .long("file")
                            .value_name("FILENAME")
                            .help("Archive file to process")
                            .required(true)
                    )
            )
            .subcommand(
                Command::new("download")
                    .about("Download a specific archive file")
                    .arg(
                        Arg::new("url")
                            .short('u')
                            .long("url")
                            .value_name("URL")
                            .help("URL to download")
                            .required(true)
                    )
                    .arg(
                        Arg::new("output")
                            .short('o')
                            .long("output")
                            .value_name("FILENAME")
                            .help("Output filename")
                            .required(true)
                    )
            )
            .subcommand(
                Command::new("status")
                    .about("Show system status")
            )
            .subcommand(
                Command::new("cleanup")
                    .about("Clean up old files and resources")
            )
    }

    async fn run_server(&mut self, matches: &ArgMatches) -> Result<()> {
        info!("Starting GitHub Archive Scraper API Server v2.0.0");

        // Update config with CLI arguments
        if let Some(port) = matches.get_one::<String>("port") {
            self.config.web.port = port.parse()?;
        }
        if let Some(host) = matches.get_one::<String>("host") {
            self.config.web.host = host.clone();
        }

        info!("Server configuration:");
        info!("  Host: {}", self.config.web.host);
        info!("  Port: {}", self.config.web.port);
        info!("  Database: {}:{}", self.config.database.host, self.config.database.port);

        // Start the API server
        let server = ApiServer::new(self.config.clone());
        server.start().await?;

        Ok(())
    }

    async fn run_scraper(&mut self, matches: &ArgMatches) -> Result<()> {
        info!("Starting GitHub Archive Scraper v2.0.0");

        let continuous = matches.get_flag("continuous");
        let max_files = matches.get_one::<String>("max-files")
            .and_then(|s| s.parse::<usize>().ok());

        info!("Scraper configuration:");
        info!("  Continuous mode: {}", continuous);
        info!("  Max files: {:?}", max_files);
        info!("  Download directory: {:?}", self.config.download.download_dir);

        // Initialize and start the main scraper
        let mut main_scraper = MainScraper::new(self.config.clone())?;
        main_scraper.initialize().await?;

        if continuous {
            info!("Starting continuous scraping...");
            main_scraper.start().await?;
        } else {
            info!("Running single scraping cycle...");
            
            // Get available files
            let files = main_scraper.get_available_files().await?;
            let files_to_process = if let Some(max) = max_files {
                &files[..max.min(files.len())]
            } else {
                &files
            };

            info!("Processing {} files", files_to_process.len());

            for file in files_to_process {
                info!("Processing: {}", file.filename);
                match main_scraper.process_single_file(&file.filename).await {
                    Ok(result) => {
                        info!("✓ {}: {} events processed", file.filename, result.valid_events);
                    }
                    Err(e) => {
                        error!("✗ {}: {}", file.filename, e);
                    }
                }
            }
        }

        main_scraper.shutdown().await?;
        Ok(())
    }

    async fn process_file(&mut self, matches: &ArgMatches) -> Result<()> {
        let filename = matches.get_one::<String>("file").unwrap();
        
        info!("Processing file: {}", filename);

        let mut main_scraper = MainScraper::new(self.config.clone())?;
        main_scraper.initialize().await?;

        match main_scraper.process_single_file(filename).await {
            Ok(result) => {
                info!("Processing completed successfully:");
                info!("  Total events: {}", result.total_events);
                info!("  Valid events: {}", result.valid_events);
                info!("  Invalid events: {}", result.invalid_events);
                info!("  Processing time: {:.2}s", result.processing_time_seconds);
                info!("  File size: {} bytes", result.file_size_bytes);
                info!("  Compression ratio: {:.2}", result.compression_ratio);
                
                if !result.event_types.is_empty() {
                    info!("  Event types:");
                    for (event_type, count) in &result.event_types {
                        info!("    {}: {}", event_type, count);
                    }
                }

                if !result.errors.is_empty() {
                    info!("  Errors encountered: {}", result.errors.len());
                    for error in result.errors.iter().take(5) {
                        info!("    {}", error);
                    }
                }
            }
            Err(e) => {
                error!("Failed to process file: {}", e);
                return Err(e);
            }
        }

        main_scraper.shutdown().await?;
        Ok(())
    }

    async fn download_file(&mut self, matches: &ArgMatches) -> Result<()> {
        let url = matches.get_one::<String>("url").unwrap();
        let output = matches.get_one::<String>("output").unwrap();

        info!("Downloading: {} -> {}", url, output);

        let mut main_scraper = MainScraper::new(self.config.clone())?;
        main_scraper.initialize().await?;

        match main_scraper.download_file(url, output).await {
            Ok(result) => {
                match result.status {
                    crate::scraper::DownloadStatus::Success => {
                        info!("Download completed successfully:");
                        info!("  Size: {} bytes", result.size_bytes);
                        info!("  Duration: {:.2}s", result.duration_seconds);
                        info!("  Retries: {}", result.retries_used);
                    }
                    crate::scraper::DownloadStatus::Failed => {
                        error!("Download failed: {}", result.error.unwrap_or_else(|| "Unknown error".to_string()));
                        return Err(anyhow::anyhow!("Download failed"));
                    }
                    crate::scraper::DownloadStatus::Skipped => {
                        info!("Download skipped (file already exists)");
                    }
                }
            }
            Err(e) => {
                error!("Download error: {}", e);
                return Err(e);
            }
        }

        main_scraper.shutdown().await?;
        Ok(())
    }

    async fn show_status(&mut self) -> Result<()> {
        info!("GitHub Archive Scraper Status");

        let mut main_scraper = MainScraper::new(self.config.clone())?;
        main_scraper.initialize().await?;

        match main_scraper.get_comprehensive_status().await {
            Ok(status) => {
                info!("System Status:");
                info!("  Running: {}", status.running);
                info!("  Uptime: {:.1}s", status.uptime_seconds);
                info!("  Files processed: {}", status.total_files_processed);
                info!("  Events processed: {}", status.total_events_processed);
                info!("  Errors: {}", status.total_errors);

                if let Some(resource_status) = status.resource_status {
                    info!("Resource Status:");
                    info!("  Memory: {:.1} GB ({:.1}%)", 
                          resource_status.memory.used_gb, 
                          resource_status.memory.percent);
                    info!("  Disk: {:.1} GB ({:.1}%)", 
                          resource_status.disk.used_gb, 
                          resource_status.disk.percent);
                    info!("  CPU: {:.1}%", resource_status.cpu.percent);
                    info!("  Emergency mode: {}", resource_status.emergency_mode);
                }

                if let Some(db_health) = status.database_health {
                    info!("Database Status:");
                    info!("  Connected: {}", db_health.is_connected);
                    info!("  Connections: {}", db_health.connection_count);
                    info!("  Active queries: {}", db_health.active_queries);
                }

                if let Some(quality_metrics) = status.quality_metrics {
                    info!("Data Quality:");
                    info!("  Total events: {}", quality_metrics.total_events);
                    info!("  Unique actors: {}", quality_metrics.unique_actors);
                    info!("  Unique repos: {}", quality_metrics.unique_repos);
                    info!("  Quality score: {:.1}", quality_metrics.quality_score);
                }
            }
            Err(e) => {
                error!("Failed to get status: {}", e);
                return Err(e);
            }
        }

        main_scraper.shutdown().await?;
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("Starting cleanup...");

        let mut main_scraper = MainScraper::new(self.config.clone())?;
        main_scraper.initialize().await?;

        let cleaned_files = main_scraper.cleanup_old_files().await?;
        info!("Cleaned {} old files", cleaned_files);

        main_scraper.shutdown().await?;
        Ok(())
    }

    async fn run_full(&mut self) -> Result<()> {
        info!("Starting GitHub Archive Scraper v2.0.0 (Full Mode)");
        info!("This will run both the API server and the scraper");

        // Initialize main scraper
        let mut main_scraper = MainScraper::new(self.config.clone())?;
        main_scraper.initialize().await?;

        // Start scraper in background
        let scraper_handle = {
            let mut scraper = main_scraper;
            tokio::spawn(async move {
                if let Err(e) = scraper.start().await {
                    error!("Scraper error: {}", e);
                }
            })
        };

        // Start API server
        let server = ApiServer::new(self.config.clone());
        let server_handle = tokio::spawn(async move {
            if let Err(e) = server.start().await {
                error!("Server error: {}", e);
            }
        });

        // Wait for either to complete (which shouldn't happen in normal operation)
        tokio::select! {
            _ = scraper_handle => {
                info!("Scraper task completed");
            }
            _ = server_handle => {
                info!("Server task completed");
            }
        }

        Ok(())
    }
}

pub fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    match CliApp::run() {
        Ok(()) => {
            info!("Application completed successfully");
        }
        Err(e) => {
            error!("Application error: {}", e);
            process::exit(1);
        }
    }
}
