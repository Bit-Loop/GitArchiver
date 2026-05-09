use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::operator::service::{
    run_ai_triage, run_bigquery_scan, run_comprehensive_hunt, run_database_operation, run_gui,
    run_performance_test, run_realtime_monitor, run_repository_scan, BigQueryScanOptions,
    ComprehensiveHuntOptions, DatabaseOperation, GuiOptions, MonitorOptions, PerformanceTest,
    RepositoryScanOptions, TriageOptions,
};

#[derive(Parser)]
#[command(name = "github-secret-hunter")]
#[command(about = "Comprehensive GitHub Secret Hunting Platform in Rust")]
#[command(version = "2.0.0")]
#[command(author = "Isaiah FPGA <isaiah.fpga@gmail.com>")]
pub struct Cli {
    #[command(subcommand)]
    command: CommandGroup,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn requires_trufflehog(&self) -> bool {
        matches!(
            self.command,
            CommandGroup::Scan(ScanGroupArgs {
                command: ScanCommand::Hunt(_) | ScanCommand::Repository(_)
            })
        )
    }

    pub async fn execute(self) -> Result<()> {
        match self.command {
            CommandGroup::Ingest(args) => execute_ingest(args).await,
            CommandGroup::Scan(args) => execute_scan(args).await,
            CommandGroup::Research(args) => execute_research(args).await,
            CommandGroup::Triage(args) => execute_triage(args).await,
            CommandGroup::Admin(args) => execute_admin(args).await,
        }
    }
}

#[derive(Subcommand)]
enum CommandGroup {
    /// Data ingestion and monitoring operations
    Ingest(IngestGroupArgs),

    /// Repository and batch scan operations
    Scan(ScanGroupArgs),

    /// Research tooling and performance exploration
    Research(ResearchGroupArgs),

    /// Finding triage workflows
    Triage(TriageGroupArgs),

    /// Administrative and maintenance operations
    Admin(AdminGroupArgs),
}

#[derive(Args)]
struct IngestGroupArgs {
    #[command(subcommand)]
    command: IngestCommand,
}

#[derive(Subcommand)]
enum IngestCommand {
    /// Run BigQuery historical ingestion
    Bigquery(BigQueryArgs),

    /// Start realtime GitHub event ingestion
    Monitor(MonitorArgs),
}

#[derive(Args)]
struct ScanGroupArgs {
    #[command(subcommand)]
    command: ScanCommand,
}

#[derive(Subcommand)]
enum ScanCommand {
    /// Start the comprehensive hunt workflow
    Hunt(HuntArgs),

    /// Scan a single repository
    Repository(RepositoryScanArgs),
}

#[derive(Args)]
struct ResearchGroupArgs {
    #[command(subcommand)]
    command: ResearchCommand,
}

#[derive(Subcommand)]
enum ResearchCommand {
    /// Launch the optional desktop GUI
    Gui(GuiArgs),

    /// Run performance experiments
    Perf(PerfArgs),
}

#[derive(Args)]
struct TriageGroupArgs {
    #[command(subcommand)]
    command: TriageCommand,
}

#[derive(Subcommand)]
enum TriageCommand {
    /// Run AI-assisted triage
    Run(TriageArgs),
}

#[derive(Args)]
struct AdminGroupArgs {
    #[command(subcommand)]
    command: AdminCommand,
}

#[derive(Subcommand)]
enum AdminCommand {
    /// Database administration
    Database(DatabaseArgs),
}

#[derive(Args)]
struct HuntArgs {
    /// Organizations to monitor
    #[arg(short, long)]
    organizations: Vec<String>,

    /// Enable BigQuery scanning
    #[arg(long)]
    bigquery: bool,

    /// Enable real-time monitoring
    #[arg(long)]
    realtime: bool,

    /// Enable AI triage
    #[arg(long)]
    ai_triage: bool,

    /// AI model path
    #[arg(long)]
    model_path: Option<String>,

    /// Database path
    #[arg(short, long, default_value = "secrets.db")]
    database: String,
}

#[derive(Args)]
struct RepositoryScanArgs {
    /// Target repository
    target: String,

    /// Scan type
    #[arg(short, long, default_value = "repository")]
    scan_type: String,

    /// Output format
    #[arg(short, long, default_value = "json")]
    output: String,
}

#[derive(Args)]
struct GuiArgs {
    /// Database path
    #[arg(short, long, default_value = "secrets.db")]
    database: String,

    /// Theme
    #[arg(short, long, default_value = "dark")]
    theme: String,
}

#[derive(Args)]
struct BigQueryArgs {
    /// GCP Project ID
    #[arg(short, long)]
    project: String,

    /// Organization to scan
    #[arg(short, long)]
    organization: Option<String>,

    /// Days back to scan
    #[arg(short, long, default_value = "30")]
    days: u32,
}

#[derive(Args)]
struct MonitorArgs {
    /// Organizations to monitor
    #[arg(short, long)]
    organizations: Vec<String>,

    /// Webhook URL
    #[arg(short, long)]
    webhook: Option<String>,

    /// Poll interval in seconds
    #[arg(long, default_value = "10")]
    interval: u64,
}

#[derive(Args)]
struct TriageArgs {
    /// Database path
    #[arg(short, long, default_value = "secrets.db")]
    database: String,

    /// Triage provider
    #[arg(long, default_value = "local-openai")]
    provider: String,

    /// OpenAI-compatible base URL
    #[arg(long)]
    base_url: Option<String>,

    /// Local OpenAI-compatible model name
    #[arg(short, long)]
    model: Option<String>,

    /// Minimum severity to triage
    #[arg(long, default_value = "medium")]
    min_severity: String,
}

#[derive(Args)]
struct DatabaseArgs {
    #[command(subcommand)]
    operation: DatabaseOperationArgs,
}

#[derive(Subcommand)]
enum DatabaseOperationArgs {
    /// Initialize database schema
    Init { path: String },

    /// Query persisted findings
    Query {
        path: String,
        #[arg(short, long)]
        limit: Option<u32>,
    },

    /// Optimize database
    Optimize { path: String },

    /// Export persisted findings
    Export {
        path: String,
        #[arg(short, long)]
        output: String,
    },

    /// Audit and repair inconsistent scan summary/queue state
    RepairScanState(RepairScanStateArgs),
}

#[derive(Args)]
struct RepairScanStateArgs {
    /// Preview affected rows without changing the database
    #[arg(long, conflicts_with = "execute")]
    dry_run: bool,

    /// Execute selected repairs
    #[arg(long, conflicts_with = "dry_run")]
    execute: bool,

    /// JSON backup path for affected rows before execution
    #[arg(long, required_if_eq("execute", "true"))]
    backup_path: Option<PathBuf>,

    /// Hard-delete secret_scans rows that claim findings but have no detections
    #[arg(long)]
    hard_delete_invalid_summaries: bool,

    /// Backfill legacy failed secret_scans rows with explicit error metadata
    #[arg(long)]
    annotate_failed_scan_errors: bool,

    /// Hard-delete queued PushEvent rows that are neither zero-commit nor forced
    #[arg(long)]
    hard_delete_invalid_queue_rows: bool,

    /// Reset due processing queue rows back to retryable pending state
    #[arg(long)]
    reset_stale_processing: bool,
}

#[derive(Args)]
struct PerfArgs {
    #[command(subcommand)]
    test: PerfTestArgs,
}

#[derive(Subcommand)]
enum PerfTestArgs {
    /// Benchmark scan processing
    Scan {
        #[arg(short, long, default_value = "1000")]
        secrets: usize,

        #[arg(short, long, default_value = "4")]
        workers: usize,
    },

    /// Benchmark database operations
    Database {
        #[arg(short, long, default_value = "secrets.db")]
        path: String,
    },

    /// Generate a performance report
    Report {
        #[arg(short, long, default_value = "report.json")]
        output: String,
    },
}

async fn execute_ingest(args: IngestGroupArgs) -> Result<()> {
    match args.command {
        IngestCommand::Bigquery(args) => {
            run_bigquery_scan(BigQueryScanOptions {
                project: args.project,
                organization: args.organization,
                days: args.days,
            })
            .await
        }
        IngestCommand::Monitor(args) => {
            run_realtime_monitor(MonitorOptions {
                organizations: args.organizations,
                webhook: args.webhook,
                interval: args.interval,
            })
            .await
        }
    }
}

async fn execute_scan(args: ScanGroupArgs) -> Result<()> {
    match args.command {
        ScanCommand::Hunt(args) => {
            run_comprehensive_hunt(ComprehensiveHuntOptions {
                organizations: args.organizations,
                bigquery: args.bigquery,
                realtime: args.realtime,
                ai_triage: args.ai_triage,
                model_path: args.model_path,
                database: args.database,
            })
            .await
        }
        ScanCommand::Repository(args) => {
            run_repository_scan(RepositoryScanOptions {
                target: args.target,
                scan_type: args.scan_type,
                output: args.output,
            })
            .await
        }
    }
}

async fn execute_research(args: ResearchGroupArgs) -> Result<()> {
    match args.command {
        ResearchCommand::Gui(args) => {
            run_gui(GuiOptions {
                database: args.database,
                theme: args.theme,
            })
            .await
        }
        ResearchCommand::Perf(args) => {
            let test = match args.test {
                PerfTestArgs::Scan { secrets, workers } => {
                    PerformanceTest::Scan { secrets, workers }
                }
                PerfTestArgs::Database { path } => PerformanceTest::Database { path },
                PerfTestArgs::Report { output } => PerformanceTest::Report { output },
            };
            run_performance_test(test).await
        }
    }
}

async fn execute_triage(args: TriageGroupArgs) -> Result<()> {
    match args.command {
        TriageCommand::Run(args) => {
            run_ai_triage(TriageOptions {
                database: args.database,
                provider: args.provider,
                base_url: args.base_url,
                model: args.model,
                min_severity: args.min_severity,
            })
            .await
        }
    }
}

async fn execute_admin(args: AdminGroupArgs) -> Result<()> {
    match args.command {
        AdminCommand::Database(args) => {
            let operation = match args.operation {
                DatabaseOperationArgs::Init { path } => DatabaseOperation::Init { path },
                DatabaseOperationArgs::Query { path, limit } => {
                    DatabaseOperation::Query { path, limit }
                }
                DatabaseOperationArgs::Optimize { path } => DatabaseOperation::Optimize { path },
                DatabaseOperationArgs::Export { path, output } => {
                    DatabaseOperation::Export { path, output }
                }
                DatabaseOperationArgs::RepairScanState(args) => {
                    let execute = args.execute;
                    if !args.dry_run && !args.execute {
                        return Err(anyhow::anyhow!(
                            "repair-scan-state requires either --dry-run or --execute"
                        ));
                    }
                    DatabaseOperation::RepairScanState {
                        execute,
                        backup_path: args.backup_path.map(|path| path.display().to_string()),
                        hard_delete_invalid_summaries: args.hard_delete_invalid_summaries,
                        annotate_failed_scan_errors: args.annotate_failed_scan_errors,
                        hard_delete_invalid_queue_rows: args.hard_delete_invalid_queue_rows,
                        reset_stale_processing: args.reset_stale_processing,
                    }
                }
            };
            run_database_operation(operation).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_group_requires_trufflehog() {
        let hunt = Cli::try_parse_from(["hunter", "scan", "hunt", "--organizations", "octo-org"])
            .expect("hunt args should parse");
        assert!(hunt.requires_trufflehog());

        let repository = Cli::try_parse_from([
            "hunter",
            "scan",
            "repository",
            "owner/repo",
            "--scan-type",
            "repository",
        ])
        .expect("repository args should parse");
        assert!(repository.requires_trufflehog());
    }

    #[test]
    fn ingest_and_admin_groups_do_not_require_trufflehog() {
        let ingest =
            Cli::try_parse_from(["hunter", "ingest", "monitor", "--organizations", "octo-org"])
                .expect("ingest args should parse");
        assert!(!ingest.requires_trufflehog());

        let admin = Cli::try_parse_from(["hunter", "admin", "database", "init", "secrets.db"])
            .expect("admin args should parse");
        assert!(!admin.requires_trufflehog());
    }
}
