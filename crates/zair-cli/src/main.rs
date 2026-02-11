//! ZAIR CLI Application

mod cli;

use clap::Parser as _;
#[cfg(feature = "prove")]
use cli::SetupCommands;
use cli::{ClaimCommands, Cli, Commands, ConfigCommands, VerifyCommands};
use zair_sdk::commands::{airdrop_claim, build_airdrop_configuration};

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
#[allow(
    clippy::too_many_lines,
    reason = "Top-level CLI dispatch keeps all command wiring in one place"
)]
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
        #[cfg(feature = "prove")]
        Commands::Setup { command } => match command {
            SetupCommands::Local {
                scheme,
                pk_out,
                vk_out,
            } => zair_sdk::commands::generate_claim_params(pk_out, vk_out, scheme).await,
        },
        Commands::Config { command } => match command {
            ConfigCommands::Build {
                config,
                pool,
                target_sapling,
                scheme_sapling,
                target_orchard,
                scheme_orchard,
                config_out,
                snapshot_out_sapling,
                snapshot_out_orchard,
            } => {
                build_airdrop_configuration(
                    config.into(),
                    pool,
                    config_out,
                    snapshot_out_sapling,
                    snapshot_out_orchard,
                    target_sapling,
                    scheme_sapling,
                    target_orchard,
                    scheme_orchard,
                )
                .await
            }
        },
        Commands::Claim { command } => match command {
            #[cfg(feature = "prove")]
            ClaimCommands::Run {
                config,
                seed,
                msg,
                snapshot_sapling,
                snapshot_orchard,
                pk,
                account,
                birthday,
                lightwalletd,
                claims_out,
                proofs_out,
                secrets_out,
                submission_out,
            } => {
                zair_sdk::commands::claim_run(
                    lightwalletd,
                    snapshot_sapling,
                    snapshot_orchard,
                    birthday,
                    claims_out,
                    proofs_out,
                    secrets_out,
                    submission_out,
                    seed,
                    account,
                    pk,
                    msg,
                    config,
                )
                .await
            }
            ClaimCommands::Prepare {
                config,
                ufvk,
                snapshot_sapling,
                snapshot_orchard,
                birthday,
                lightwalletd,
                claims_out,
            } => {
                airdrop_claim(
                    lightwalletd,
                    snapshot_sapling,
                    snapshot_orchard,
                    ufvk,
                    birthday,
                    claims_out,
                    config,
                )
                .await
            }
            #[cfg(feature = "prove")]
            ClaimCommands::Prove {
                config,
                claims_in,
                seed,
                pk,
                account,
                proofs_out,
                secrets_out,
            } => {
                zair_sdk::commands::generate_claim_proofs(
                    claims_in,
                    proofs_out,
                    seed,
                    account,
                    pk,
                    secrets_out,
                    config,
                )
                .await
            }
            ClaimCommands::Sign {
                config,
                proofs_in,
                secrets_in,
                seed,
                msg,
                account,
                submission_out,
            } => {
                zair_sdk::commands::sign_claim_submission(
                    proofs_in,
                    secrets_in,
                    seed,
                    account,
                    config,
                    msg,
                    submission_out,
                )
                .await
            }
        },
        Commands::Verify { command } => match command {
            VerifyCommands::Run {
                config,
                vk,
                submission_in,
                msg,
            } => zair_sdk::commands::verify_run(vk, submission_in, msg, config).await,
            VerifyCommands::Proof {
                config,
                vk,
                proofs_in,
            } => zair_sdk::commands::verify_claim_sapling_proof(proofs_in, vk, config).await,
            VerifyCommands::Signature {
                config,
                submission_in,
                msg,
            } => {
                zair_sdk::commands::verify_claim_submission_signature(submission_in, msg, config)
                    .await
            }
        },
    };

    if let Err(e) = res {
        tracing::error!("Error: {:?}", e);
        std::process::exit(1);
    }

    Ok(())
}
