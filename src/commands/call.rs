use anyhow::{Result, anyhow, bail};
use clap::Args;
use pit_lane_core::PitUri;
use reqwest::Client;
use std::str::FromStr;

#[derive(Debug, Args)]
pub struct CallArgs {
    /// HTTP method (GET by default).
    #[arg(long, default_value = "GET")]
    pub method: String,
    /// Request header, repeatable as NAME: VALUE.
    #[arg(long = "header", short = 'H')]
    pub headers: Vec<String>,
    /// UTF-8 request body.
    #[arg(long)]
    pub body: Option<String>,
    /// PitLane endpoint; defaults to PIT_LANE_ENDPOINT or 127.0.0.1:7080.
    #[arg(long)]
    pub endpoint: Option<String>,
    /// Logical service URI, for example pit://service-a/hello.
    pub uri: String,
}

pub async fn run(args: CallArgs) -> Result<()> {
    let logical = PitUri::from_str(&args.uri)?;
    let endpoint = args
        .endpoint
        .or_else(|| std::env::var("PIT_LANE_ENDPOINT").ok())
        .unwrap_or_else(|| "http://127.0.0.1:7080".into());
    let endpoint = endpoint.trim_end_matches('/');
    let url = format!("{endpoint}{}", logical.path_and_query);
    let client = Client::new();
    let mut request = client
        .request(args.method.parse()?, url)
        .header("X-Pit-Service", logical.service.as_str());
    for header in args.headers {
        let (name, value) = header
            .split_once(':')
            .ok_or_else(|| anyhow!("header must use NAME: VALUE"))?;
        request = request.header(name.trim(), value.trim());
    }
    if let Some(body) = args.body {
        request = request.body(body);
    }
    let response = request.send().await?;
    let status = response.status();
    let body = response.bytes().await?;
    print!("{}", String::from_utf8_lossy(&body));
    if !body.ends_with(b"\n") {
        println!();
    }
    if !status.is_success() {
        bail!("PitLane returned HTTP {status}")
    }
    Ok(())
}
