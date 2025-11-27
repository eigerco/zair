//! Airdrop CLI Application

use clap::Parser as _;
use non_membership_proofs::{build_merkle_tree, partition_by_pool, write_raw_nullifiers};
use rs_merkle::algorithms::Sha256;
use tracing::debug;

use crate::cli::{Cli, Commands, CommonArgs};

mod airdrop_configuration;
mod chain_nullifiers;
mod cli;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Initialize rustls crypto provider (required for TLS connections)
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Load .env file (fails silently if not found)
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Note: .env file not loaded: {e}");
    } else {
        eprintln!("Loaded .env file");
    }

    // Debug: show current RUST_LOG setting
    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        eprintln!("RUST_LOG is set to: {rust_log}");
    } else {
        eprintln!("RUST_LOG is not set, using default: info");
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Parse CLI arguments (includes env vars loaded from .env)
    let cli = Cli::parse();
    debug!("Cli Configuration: {cli:?}");

    match &cli.command {
        Commands::BuildAirdropConfiguration {
            config,
            configuration_output_file,
        } => {
            let stream = chain_nullifiers::get_nullifiers(&config).await?;

            let (mut sapling, mut orchard) = partition_by_pool(stream).await?;

            // store nullifiers
            // Store the nullifiers so we can later generate proofs for
            // the nullifiers we are interested in.
            write_raw_nullifiers(&sapling, "sapling_nullifiers-runtime.bin").await?;
            write_raw_nullifiers(&orchard, "orchard_nullifiers-runtime.bin").await?;

            let sapling_tree = build_merkle_tree::<Sha256>(&mut sapling);
            let orchard_tree = build_merkle_tree::<Sha256>(&mut orchard);

            airdrop_configuration::AirdropConfiguration::new(
                sapling_tree.root_hex().as_deref(),
                orchard_tree.root_hex().as_deref(),
            )
            .export_config(configuration_output_file)
            .await?;

            Ok(())
        }
        Commands::FindNotes { .. } => {
            todo!()
        }
    }
}
