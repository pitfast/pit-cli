use anyhow::Result;
use pit_scheduler::PitScheduler;

pub fn run() -> Result<()> {
    let lanes = PitScheduler::local().lane_count();
    println!("PitFast Local System");
    println!();
    println!("Architecture: {}", std::env::consts::ARCH);
    println!("Execution lanes: {lanes}");
    println!("Parallelism source: std::thread::available_parallelism()");
    Ok(())
}
