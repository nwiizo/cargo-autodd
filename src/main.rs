use anyhow::Result;
use cargo_autodd::CargoAutodd;
use clap::{Parser, Subcommand};
use std::env;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "cargo-autodd",
    version,
    about = "Automatically manages dependencies in your Rust projects"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze and update dependencies
    Autodd(AutoddArgs),
}

#[derive(clap::Args)]
struct AutoddArgs {
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,

    /// Preview changes without modifying files
    #[arg(long)]
    dry_run: bool,

    /// Path to config file (default: .cargo-autodd.toml)
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    subcommand: Option<AutoddSubcommand>,
}

#[derive(Subcommand)]
enum AutoddSubcommand {
    /// Update dependencies to latest versions
    Update,
    /// Generate dependency usage report
    Report,
    /// Check for security vulnerabilities
    Security,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let Commands::Autodd(args) = cli.command;

    let current_dir = env::current_dir()?;

    // Load config
    let config = if let Some(config_path) = &args.config {
        cargo_autodd::Config::load(config_path)?
    } else {
        cargo_autodd::Config::load_default(&current_dir)?
    };

    let autodd = CargoAutodd::with_options(current_dir, args.debug, args.dry_run, config);

    // Handle subcommands
    match args.subcommand {
        Some(AutoddSubcommand::Update) => {
            println!("Updating dependencies to latest versions...");
            autodd.update_dependencies()?;
        }
        Some(AutoddSubcommand::Report) => {
            println!("Generating dependency usage report...");
            autodd.generate_report()?;
        }
        Some(AutoddSubcommand::Security) => {
            println!("Checking for security vulnerabilities...");
            autodd.check_security()?;
        }
        None => {
            // Default behavior: analyze and update
            autodd.analyze_and_update()?;
        }
    }

    Ok(())
}
