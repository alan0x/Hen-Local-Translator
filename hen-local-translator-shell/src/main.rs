//! Hen Local Live Translator - Standalone Live Translation Application
//!
//! A standalone desktop application for live speech translation.

mod app;
mod dataflow;
mod preferences;
mod runtime;

use clap::Parser;

#[derive(Parser, Debug, Default, Clone)]
#[command(name = "hen-local-translator")]
#[command(about = "Hen Local Live Translator")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Args {
    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    pub log_level: String,

    /// Dora dataflow YAML file path
    #[arg(short, long)]
    pub dataflow: Option<String>,
}

impl Args {
    /// Get the log filter string for env_logger
    pub fn log_filter(&self) -> &str {
        &self.log_level
    }
}

fn main() {
    // Parse command-line arguments
    let args = Args::parse();

    // Configure logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(args.log_filter()))
        .init();

    log::info!(
        "Starting Hen Local Live Translator v{}",
        env!("CARGO_PKG_VERSION")
    );
    log::debug!("CLI args: {:?}", args);

    if let Some(ref dataflow) = args.dataflow {
        log::info!("Using dataflow: {}", dataflow);
    }

    app::run(args);
}
