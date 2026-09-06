use anyhow::{Context, Result};
use clap::Args;
use pit_circuit_core::{CircuitSnapshot, GarageId};

#[derive(Debug, Args)]
pub struct EndpointArgs {
    #[arg(long, default_value = "http://127.0.0.1:7090")]
    pub endpoint: String,
}

#[derive(Debug, Args)]
pub struct GarageInspectArgs {
    pub id: GarageId,
    #[arg(long, default_value = "http://127.0.0.1:7090")]
    pub endpoint: String,
}

async fn get_snapshot(endpoint: &str) -> Result<CircuitSnapshot> {
    reqwest::get(format!("{}/v1/snapshot", endpoint.trim_end_matches('/')))
        .await
        .context("Circuit request failed")?
        .error_for_status()
        .context("Circuit returned an error")?
        .json()
        .await
        .context("Circuit returned invalid JSON")
}

pub async fn status(args: EndpointArgs) -> Result<()> {
    let snapshot = get_snapshot(&args.endpoint).await?;
    println!("Circuit: {}", snapshot.circuit_id);
    println!("Version: {}", snapshot.version);
    println!("Garages: {}", snapshot.garages.len());
    Ok(())
}

pub async fn snapshot(args: EndpointArgs) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&get_snapshot(&args.endpoint).await?)?
    );
    Ok(())
}

pub async fn list(args: EndpointArgs) -> Result<()> {
    let snapshot = get_snapshot(&args.endpoint).await?;
    println!("GARAGE      STATUS    LANES  ACTIVE  QUEUE");
    for garage in snapshot.garages {
        println!(
            "{:<10}  {:<8}  {:>5}  {:>6}  {:>5}",
            garage.id,
            format!("{:?}", garage.health).to_lowercase(),
            garage.total_lanes,
            garage.active_lanes,
            garage.queue_depth
        );
    }
    Ok(())
}

pub async fn inspect(args: GarageInspectArgs) -> Result<()> {
    let snapshot = get_snapshot(&args.endpoint).await?;
    let garage = snapshot
        .garages
        .into_iter()
        .find(|garage| garage.id == args.id)
        .with_context(|| format!("unknown Garage '{}'", args.id))?;
    println!("Garage");
    println!("  ID: {}", garage.id);
    println!("  Status: {:?}", garage.health);
    println!("  Endpoint: {}", garage.endpoint);
    println!();
    println!("Grid");
    println!("  Lanes: {}", garage.total_lanes);
    println!("  Active: {}", garage.active_lanes);
    println!("  Free: {}", garage.free_lanes);
    println!("  Queue: {}", garage.queue_depth);
    println!();
    println!("Artifacts");
    println!("  Local: {}", garage.local_artifacts.len());
    println!("  Prepared: {}", garage.prepared_artifacts.len());
    Ok(())
}
