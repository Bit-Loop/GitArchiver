use anyhow::Result;
use github_archiver::operator::cli::Cli;
use github_archiver::operator::service::ensure_trufflehog_ready;
use tracing::info;

fn init_logging(verbose: bool) {
    let log_level = if verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("github_archiver={}", log_level))
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();

    init_logging(cli.verbose);
    info!("🔍 GitHub Secret Hunter v2.0.0 starting...");

    if cli.requires_trufflehog() {
        ensure_trufflehog_ready()?;
    }

    cli.execute().await
}
