use anyhow::Result;
use cargo_autodd::CargoAutodd;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // When cargo-autodd is called as 'cargo autodd',
    // the first argument is "cargo" and the second is "autodd"
    if args.len() > 1 && args[1] != "autodd" {
        println!("This command should be run as 'cargo autodd'");
        std::process::exit(1);
    }

    let debug = args.iter().any(|arg| arg == "--debug" || arg == "-d");
    let current_dir = env::current_dir()?;

    let autodd = CargoAutodd::with_debug(current_dir, debug);
    autodd.analyze_and_update()?;

    Ok(())
}
