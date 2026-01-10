//! Airdrop CLI Application

use clap::Parser as _;
use non_membership_proofs::user_nullifiers::{OrchardViewingKeys, SaplingViewingKeys, ViewingKeys};
use zcash_keys::keys::UnifiedFullViewingKey;

use crate::cli::{Cli, Commands};
use crate::commands::{
    airdrop_claim, airdrop_configuration_schema, build_airdrop_configuration, construct_proofs,
};

mod chain_nullifiers;
mod cli;
mod commands;
mod unspent_notes_proofs;

pub(crate) const BUF_SIZE: usize = 1024 * 1024;

fn init_tracing() -> eyre::Result<()> {
    #[cfg(feature = "tokio-console")]
    {
        // tokio-console: layers the console subscriber with fmt
        use tracing_subscriber::prelude::*;
        tracing_subscriber::registry()
            .with(console_subscriber::spawn())
            .with(
                tracing_subscriber::fmt::layer().with_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                ),
            )
            .try_init()
            .map_err(|e| eyre::eyre!("Failed to initialize tracing: {:?}", e))?;
    }

    #[cfg(not(feature = "tokio-console"))]
    {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_timer(tracing_subscriber::fmt::time::uptime())
            .with_target(false)
            .try_init()
            .map_err(|e| eyre::eyre!("Failed to initialize tracing: {:?}", e))?;
    }

    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> eyre::Result<()> {
    // Initialize rustls crypto provider (required for TLS connections)
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|e| eyre::eyre!("Failed to install rustls crypto provider: {e:?}"))?;

    // Load .env file (fails silently if not found)
    let _ = dotenvy::dotenv();

    init_tracing()?;

    let cli = Cli::parse();

    let res = match cli.command {
        Commands::BuildAirdropConfiguration {
            config,
            configuration_output_file,
            sapling_snapshot_nullifiers,
            orchard_snapshot_nullifiers,
            hiding_factor,
        } => {
            build_airdrop_configuration(
                config,
                configuration_output_file,
                sapling_snapshot_nullifiers,
                orchard_snapshot_nullifiers,
                hiding_factor.into(),
            )
            .await
        }
        Commands::AirdropClaim {
            config,
            sapling_snapshot_nullifiers,
            orchard_snapshot_nullifiers,
            unified_full_viewing_key,
            birthday_height,
            airdrop_claims_output_file,
            airdrop_configuration_file,
        } => {
            let ufvk = UnifiedFullViewingKey::decode(&config.network, &unified_full_viewing_key)
                .map_err(|e| eyre::eyre!("Failed to decode Unified Full Viewing Key: {:?}", e))?;

            let orchard_fvk = ufvk.orchard().ok_or_else(|| {
                eyre::eyre!("Unified Full Viewing Key does not contain an Orchard FVK")
            })?;

            let sapling_fvk = ufvk.sapling().ok_or_else(|| {
                eyre::eyre!("Unified Full Viewing Key does not contain a Sapling FVK")
            })?;

            let viewing_keys = ViewingKeys {
                sapling: Some(SaplingViewingKeys::from_dfvk(sapling_fvk)),
                orchard: Some(OrchardViewingKeys::from_fvk(orchard_fvk)),
            };

            airdrop_claim(
                config,
                sapling_snapshot_nullifiers,
                orchard_snapshot_nullifiers,
                viewing_keys,
                birthday_height,
                airdrop_claims_output_file,
                airdrop_configuration_file,
            )
            .await
        }
        Commands::AirdropConfigurationSchema => airdrop_configuration_schema(),
        Commands::ConstructProofs {
            airdrop_claims_input_file,
        } => construct_proofs(airdrop_claims_input_file).await,
    };

    if let Err(e) = res {
        tracing::error!("Error: {:?}", e);
        std::process::exit(1);
    }

    Ok(())
}
