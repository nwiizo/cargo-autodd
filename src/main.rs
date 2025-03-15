use anyhow::Result;
use cargo_autodd::CargoAutodd;
use clap::{App, Arg, SubCommand};
use std::env;

fn main() -> Result<()> {
    let matches = App::new("cargo-autodd")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Automatically manages dependencies in your Rust projects")
        .subcommand(
            SubCommand::with_name("autodd")
                .about("Analyze and update dependencies")
                .arg(
                    Arg::with_name("debug")
                        .short("d")
                        .long("debug")
                        .help("Enable debug output"),
                )
                .subcommand(
                    SubCommand::with_name("update").about("Update dependencies to latest versions"),
                )
                .subcommand(
                    SubCommand::with_name("report").about("Generate dependency usage report"),
                )
                .subcommand(
                    SubCommand::with_name("security").about("Check for security vulnerabilities"),
                ),
        )
        .get_matches();

    // When cargo-autodd is called directly (not as a cargo subcommand)
    if env::args().nth(1) != Some("autodd".to_string()) {
        println!("This command should be run as 'cargo autodd'");
        std::process::exit(1);
    }

    // Get the autodd subcommand matches
    let autodd_matches = matches.subcommand_matches("autodd").unwrap_or_else(|| {
        println!("Missing 'autodd' subcommand. Run 'cargo autodd --help' for usage information.");
        std::process::exit(1);
    });

    let debug = autodd_matches.is_present("debug");
    let current_dir = env::current_dir()?;
    let autodd = CargoAutodd::with_debug(current_dir, debug);

    // Handle subcommands
    match autodd_matches.subcommand_name() {
        Some("update") => {
            println!("Updating dependencies to latest versions...");
            autodd.update_dependencies()?;
        }
        Some("report") => {
            println!("Generating dependency usage report...");
            autodd.generate_report()?;
        }
        Some("security") => {
            println!("Checking for security vulnerabilities...");
            autodd.check_security()?;
        }
        _ => {
            // Default behavior: analyze and update
            autodd.analyze_and_update()?;
        }
    }

    Ok(())
}
