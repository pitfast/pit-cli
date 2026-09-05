use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Args;
use pit_node::{ExecutionRequest, ExecutionResult, PitNode, WasmArtifact};

use super::run::{format_duration, load_managed_artifact, resolve_artifact};

#[derive(Debug, Args)]
pub struct BenchArgs {
    /// Path to the WASM module. Omit it to use .pit/artifact.json.
    pub wasm_file: Option<PathBuf>,
}

pub async fn run(args: BenchArgs) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let (wasm_file, entrypoint) = if args.wasm_file.is_some() {
        (
            resolve_artifact(&project_dir, args.wasm_file.as_deref()).await?,
            None,
        )
    } else {
        let (manifest, path) = load_managed_artifact(&project_dir)?;
        (path, Some(manifest.runtime.entrypoint.as_str().to_owned()))
    };
    benchmark(wasm_file, entrypoint)
}

fn benchmark(wasm_file: PathBuf, entrypoint: Option<String>) -> Result<()> {
    let preparation_started = std::time::Instant::now();
    let node = PitNode::from_file(&wasm_file)?;
    let preparation = preparation_started.elapsed();
    let artifact = WasmArtifact::from_path(&wasm_file);
    let entrypoint = entrypoint.unwrap_or_else(|| node.default_entrypoint().to_owned());
    let request = ExecutionRequest::new(artifact.clone()).with_entrypoint(entrypoint);
    let instantiation = node.measure_instantiation(&request)?;
    let levels = benchmark_levels(node.execution_lanes());

    println!("PitFast Benchmark");
    println!();
    println!("Artifact:");
    println!("  {}", wasm_file.display());
    println!();
    println!("Execution lanes:");
    println!("  {}", node.execution_lanes());
    println!("Artifact preparation:");
    println!("  {}", format_duration(preparation));
    println!("Instantiation:");
    println!("  {}", format_duration(instantiation));
    println!();
    println!(
        "{:<12} {:>10} {:>14} {:>10} {:>10} {:>10} {:>10} {:>6}",
        "Concurrency", "Total", "Throughput", "Mean", "p50", "p95", "p99", "Peak"
    );

    for concurrency in levels {
        let report = node.execute_many(request.clone(), concurrency)?;
        let timing = TimingSummary::from_reports(&report.executions);
        println!(
            "{:<12} {:>10} {:>13.2}/s {:>9} {:>9} {:>9} {:>9} {:>6}",
            concurrency,
            format_duration(report.total_duration),
            timing.throughput(concurrency, report.total_duration),
            format_duration(timing.mean),
            format_duration(timing.p50),
            format_duration(timing.p95),
            format_duration(timing.p99),
            report.peak_active,
        );
        if report.completed != report.requested {
            anyhow::bail!("benchmark execution failed at concurrency {concurrency}");
        }
    }
    Ok(())
}

fn benchmark_levels(lanes: usize) -> Vec<usize> {
    let mut levels = vec![1];
    let mut level = 2;
    while level < lanes {
        levels.push(level);
        level = level.saturating_mul(2);
        if level == usize::MAX {
            break;
        }
    }
    if !levels.contains(&lanes) {
        levels.push(lanes);
    }
    let oversubscribed = lanes.saturating_mul(2).max(lanes.saturating_add(1));
    if !levels.contains(&oversubscribed) {
        levels.push(oversubscribed);
    }
    levels
}

#[derive(Debug, Clone, Copy)]
struct TimingSummary {
    mean: std::time::Duration,
    p50: std::time::Duration,
    p95: std::time::Duration,
    p99: std::time::Duration,
}

impl TimingSummary {
    fn from_reports(reports: &[ExecutionResult]) -> Self {
        let mut durations = reports
            .iter()
            .map(|report| report.duration)
            .collect::<Vec<_>>();
        durations.sort_unstable();
        let total_nanos = durations
            .iter()
            .map(std::time::Duration::as_nanos)
            .sum::<u128>();
        let mean = if durations.is_empty() {
            std::time::Duration::ZERO
        } else {
            std::time::Duration::from_nanos(
                u64::try_from(total_nanos / durations.len() as u128).unwrap_or(u64::MAX),
            )
        };
        Self {
            mean,
            p50: percentile(&durations, 50),
            p95: percentile(&durations, 95),
            p99: percentile(&durations, 99),
        }
    }

    fn throughput(self, executions: usize, total: std::time::Duration) -> f64 {
        executions as f64 / total.as_secs_f64()
    }
}

fn percentile(sorted: &[std::time::Duration], percentile: usize) -> std::time::Duration {
    if sorted.is_empty() {
        return std::time::Duration::ZERO;
    }
    let rank = percentile.saturating_mul(sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

#[allow(dead_code)]
fn _path_is_explicit(project_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_dir.join(path)
    }
}
