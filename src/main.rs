use anyhow::Result;
use cargo_autodd::CargoAutodd;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    
    // cargo-autodd が cargo autodd として呼び出された場合、
    // 最初の引数は "cargo" で2番目が "autodd" になります
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
