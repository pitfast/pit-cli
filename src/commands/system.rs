use anyhow::Result;
use pit_scheduler::PitScheduler;

pub fn run() -> Result<()> {
    let lanes = PitScheduler::local().lane_count();
    println!("PitFast Local System");
    println!();
    println!("Architecture: {}", std::env::consts::ARCH);
    println!("Execution lanes: {lanes}");
    println!("Parallelism source: std::thread::available_parallelism()");
    println!();
    println!("Runtime ABIs");
    println!("  wasi-preview1  supported");
    println!("  wasi-preview2  supported");
    println!();
    println!("Canonical ABI: wasi-preview2");
    println!();
    println!("Component Worlds");
    println!("  wasi:cli/command  supported");
    println!("  wasi:http/proxy   supported");
    Ok(())
}
