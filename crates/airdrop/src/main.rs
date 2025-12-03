//! Airdrop CLI Application

use clap::Parser as _;
use non_membership_proofs::{
    build_merkle_tree, partition_by_pool, read_raw_nullifiers, write_raw_nullifiers,
};
use rs_merkle::algorithms::Sha256;
use tracing::info;

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
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .init();

    // Parse CLI arguments (includes env vars loaded from .env)
    let cli = Cli::parse();
    info!("Cli Configuration: {cli:?}");

    match &cli.command {
        Commands::BuildAirdropConfiguration {
            config,
            configuration_output_file,
            sapling_snapshot_nullifiers,
            orchard_snapshot_nullifiers,
        } => {
            let stream = chain_nullifiers::get_nullifiers(&config).await?;

            let (mut sapling_nullifiers, mut orchard_nullifiers) =
                partition_by_pool(stream).await?;

            info!(
                "Collected {} sapling nullifiers and {} orchard nullifiers",
                sapling_nullifiers.len(),
                orchard_nullifiers.len()
            );

            // store nullifiers
            // Store the nullifiers so we can later generate proofs for
            // the nullifiers we are interested in.
            write_raw_nullifiers(&sapling_nullifiers, sapling_snapshot_nullifiers).await?;
            info!("Written sapling nullifiers to disk");
            write_raw_nullifiers(&orchard_nullifiers, orchard_snapshot_nullifiers).await?;
            info!("Written orchard nullifiers to disk");

            let sapling_tree = build_merkle_tree::<Sha256>(&mut sapling_nullifiers);
            info!(
                "Built sapling merkle tree with root: {}",
                sapling_tree.root_hex().unwrap_or_default()
            );

            let orchard_tree = build_merkle_tree::<Sha256>(&mut orchard_nullifiers);
            info!(
                "Built orchard merkle tree with root: {}",
                orchard_tree.root_hex().unwrap_or_default()
            );

            airdrop_configuration::AirdropConfiguration::new(
                sapling_tree.root_hex().as_deref(),
                orchard_tree.root_hex().as_deref(),
            )
            .export_config(configuration_output_file)
            .await?;

            info!("Exported airdrop configuration to {configuration_output_file}",);

            Ok(())
        }
        Commands::FindNotes {
            config: _,
            sapling_snapshot_nullifiers,
            orchard_snapshot_nullifiers,
            orchard_fvk: _,
            sapling_fvk: _,
        } => {
            // TODO: if the sapling or orchard snapshot nullifiers files do not exist,
            // it should be possible to build them from the chain again.
            let mut sapling_nullifiers = read_raw_nullifiers(sapling_snapshot_nullifiers).await?;
            let mut orchard_nullifiers = read_raw_nullifiers(orchard_snapshot_nullifiers).await?;

            let sapling_tree = build_merkle_tree::<Sha256>(&mut sapling_nullifiers);
            info!(
                "Built sapling merkle tree with root: {}",
                sapling_tree.root_hex().unwrap_or_default()
            );

            let orchard_tree = build_merkle_tree::<Sha256>(&mut orchard_nullifiers);
            info!(
                "Built orchard merkle tree with root: {}",
                orchard_tree.root_hex().unwrap_or_default()
            );

            // Find user notes logic

            Ok(())
        }
    }
}
