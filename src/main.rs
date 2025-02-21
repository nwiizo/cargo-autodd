use std::env;
use std::path::PathBuf;
use anyhow::Result;
use cargo_autodd::CargoAutodd;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let debug = args.iter().any(|arg| arg == "--debug" || arg == "-d");
    let current_dir = env::current_dir()?;
    
    let autodd = CargoAutodd::with_debug(current_dir, debug);
    autodd.analyze_and_update()?;
    
    Ok(())
}
