use anyhow::Result;
use clap::Parser;
use normalize_addresses::normalize_async;
use serde_json::json;

/// Simple CLI wrapper around the Rust port of normalize-japanese-addresses.
#[derive(Parser, Debug)]
#[command(author, version, about = "Normalize Japanese addresses offline", long_about = None)]
struct Args {
    /// Address string to normalize
    address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let normalized = normalize_async(&args.address).await?;
    let output = json!({
        "pref": normalized.pref,
        "city": normalized.city,
        "town": normalized.town,
        "addr": normalized.addr,
        "level": normalized.level,
        "point": normalized.point,
        "other": normalized.other,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
