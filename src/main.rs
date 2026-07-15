use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

mod config;
mod error;
mod executor;
mod nut;
mod publisher;
mod subscriber;

/// MQTT connector for NUT protocol — lightweight UPS power-outage shutdown daemon.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the TOML configuration file.
    #[arg(short, long, default_value = "/etc/mqnutd/config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise logging (RUST_LOG controls verbosity, default: info).
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let raw = std::fs::read_to_string(&args.config)
        .map_err(|e| anyhow::anyhow!("Cannot read config file {:?}: {}", args.config, e))?;

    let cfg: config::Config = toml::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("Config parse error: {}", e))?;

    tracing::info!(mode = ?cfg.mode, "mqnutd starting");

    match cfg.mode {
        config::Mode::Publisher => publisher::run(&cfg).await?,
        config::Mode::Subscriber => subscriber::run(&cfg).await?,
    }

    Ok(())
}

