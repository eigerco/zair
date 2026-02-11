//! Command-line interface for the `zair` CLI application.

use std::path::PathBuf;

use clap::Parser;
use eyre::{Result, ensure, eyre};
use zair_core::schema::config::ValueCommitmentScheme;
#[cfg(feature = "prove")]
use zair_sdk::commands::SaplingSetupScheme;
use zair_sdk::common::{CommonConfig, PoolSelection};
use zcash_protocol::consensus::Network;

/// Command-line interface definition.
#[derive(Debug, Parser)]
#[command(name = "zair")]
#[command(about = "Zcash airdrop tools")]
pub struct Cli {
    /// CLI top-level command group.
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level command groups.
#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    /// Setup utilities (organizer/developer focused).
    #[cfg(feature = "prove")]
    Setup {
        /// Setup subcommands.
        #[command(subcommand)]
        command: SetupCommands,
    },
    /// Airdrop configuration utilities.
    Config {
        /// Config subcommands.
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Claim pipeline commands.
    Claim {
        /// Claim subcommands.
        #[command(subcommand)]
        command: ClaimCommands,
    },
    /// Verification pipeline commands.
    Verify {
        /// Verify subcommands.
        #[command(subcommand)]
        command: VerifyCommands,
    },
}

/// Setup command group.
#[cfg(feature = "prove")]
#[derive(Debug, clap::Subcommand)]
pub enum SetupCommands {
    /// Generate claim circuit parameters (proving and verifying keys).
    Local {
        /// Sapling circuit scheme to generate params for.
        #[arg(
            long,
            env = "SETUP_SCHEME",
            default_value = "native",
            value_parser = parse_setup_scheme
        )]
        scheme: SaplingSetupScheme,

        /// Output file for proving key.
        #[arg(long, env = "SETUP_PK_OUT", default_value = "setup-sapling-pk.params")]
        pk_out: PathBuf,

        /// Output file for verifying key.
        #[arg(long, env = "SETUP_VK_OUT", default_value = "setup-sapling-vk.params")]
        vk_out: PathBuf,
    },
}

/// Config command group.
#[derive(Debug, clap::Subcommand)]
pub enum ConfigCommands {
    /// Build a snapshot of nullifiers from a source.
    Build {
        /// Build-config specific arguments.
        #[command(flatten)]
        config: BuildConfigArgs,
        /// Pool to include in the exported configuration.
        #[arg(long, env = "POOL", default_value = "both", value_parser = parse_pool_selection)]
        pool: PoolSelection,
        /// Sapling target id used for hiding nullifier derivation. Must be exactly 8 bytes.
        #[arg(
            long,
            env = "TARGET_SAPLING",
            default_value = "ZAIRTEST",
            value_parser = parse_sapling_target_id
        )]
        target_sapling: String,
        /// Sapling value commitment scheme.
        #[arg(
            long,
            env = "SCHEME_SAPLING",
            default_value = "native",
            value_parser = parse_value_commitment_scheme
        )]
        scheme_sapling: ValueCommitmentScheme,
        /// Orchard target id used for hiding nullifier derivation. Must be <= 32 bytes.
        #[arg(
            long,
            env = "TARGET_ORCHARD",
            default_value = "ZAIRTEST:O",
            value_parser = parse_orchard_target_id
        )]
        target_orchard: String,
        /// Orchard value commitment scheme.
        #[arg(
            long,
            env = "SCHEME_ORCHARD",
            default_value = "native",
            value_parser = parse_value_commitment_scheme
        )]
        scheme_orchard: ValueCommitmentScheme,
        /// Configuration output file.
        #[arg(long, env = "CONFIG_OUT", default_value = "config.json")]
        config_out: PathBuf,
        /// Sapling snapshot nullifiers output file.
        #[arg(
            long,
            env = "SNAPSHOT_OUT_SAPLING",
            default_value = "snapshot-sapling.bin"
        )]
        snapshot_out_sapling: PathBuf,
        /// Orchard snapshot nullifiers output file.
        #[arg(
            long,
            env = "SNAPSHOT_OUT_ORCHARD",
            default_value = "snapshot-orchard.bin"
        )]
        snapshot_out_orchard: PathBuf,
    },
}

/// Claim command group.
#[derive(Debug, clap::Subcommand)]
pub enum ClaimCommands {
    /// Recommended end-to-end claim pipeline:
    /// `prepare -> prove -> sign`.
    #[cfg(feature = "prove")]
    Run {
        /// Airdrop configuration file.
        #[arg(
            long,
            env = "CONFIG_FILE",
            value_name = "CONFIG_FILE",
            default_value = "config.json"
        )]
        config: PathBuf,
        /// Path to file containing 64-byte seed as hex.
        #[arg(long, env = "SEED_FILE", value_name = "SEED_FILE")]
        seed: PathBuf,
        /// Message payload file to bind into submission signatures.
        #[arg(
            long,
            env = "MESSAGE_FILE",
            value_name = "MESSAGE_FILE",
            default_value = "claim-message.bin"
        )]
        msg: PathBuf,
        /// Sapling snapshot nullifiers file.
        /// Defaults to `snapshot-sapling.bin` when Sapling is enabled in config.
        #[arg(long, env = "SNAPSHOT_SAPLING_FILE")]
        snapshot_sapling: Option<PathBuf>,
        /// Orchard snapshot nullifiers file.
        /// Defaults to `snapshot-orchard.bin` when Orchard is enabled in config.
        #[arg(long, env = "SNAPSHOT_ORCHARD_FILE")]
        snapshot_orchard: Option<PathBuf>,
        /// Path to proving key file.
        #[arg(
            long,
            env = "PROVING_KEY_FILE",
            value_name = "PROVING_KEY_FILE",
            default_value = "setup-sapling-pk.params"
        )]
        pk: PathBuf,
        /// ZIP-32 account index used to derive Sapling keys from the seed.
        #[arg(long, env = "ACCOUNT_ID", default_value_t = 0_u32)]
        account: u32,
        /// Scan start height for note discovery.
        #[arg(long, env = "BIRTHDAY")]
        birthday: u64,
        /// Optional lightwalletd gRPC endpoint URL override.
        #[arg(long, env = "LIGHTWALLETD_URL")]
        lightwalletd: Option<String>,
        /// Output file for prepared claims JSON.
        #[arg(long, env = "CLAIMS_OUT", default_value = "claim-prepared.json")]
        claims_out: PathBuf,
        /// Output file for generated proofs.
        #[arg(long, env = "PROOFS_OUT", default_value = "claim-proofs.json")]
        proofs_out: PathBuf,
        /// Output file for local-only claim secrets.
        #[arg(long, env = "SECRETS_OUT", default_value = "claim-proofs-secrets.json")]
        secrets_out: PathBuf,
        /// Output file for signed claim submission bundle.
        #[arg(long, env = "SUBMISSION_OUT", default_value = "claim-submission.json")]
        submission_out: PathBuf,
    },
    /// Prepare the airdrop claim.
    #[command(verbatim_doc_comment)]
    Prepare {
        /// Airdrop configuration file.
        #[arg(
            long,
            env = "CONFIG_FILE",
            value_name = "CONFIG_FILE",
            default_value = "config.json"
        )]
        config: PathBuf,
        /// Unified Full Viewing Key to scan for notes.
        #[arg(long, env = "UFVK")]
        ufvk: String,
        /// Sapling snapshot nullifiers file.
        /// Defaults to `snapshot-sapling.bin` when Sapling is enabled in config.
        #[arg(long, env = "SNAPSHOT_SAPLING_FILE")]
        snapshot_sapling: Option<PathBuf>,
        /// Orchard snapshot nullifiers file.
        /// Defaults to `snapshot-orchard.bin` when Orchard is enabled in config.
        #[arg(long, env = "SNAPSHOT_ORCHARD_FILE")]
        snapshot_orchard: Option<PathBuf>,
        /// Scan start height for note discovery.
        #[arg(long, env = "BIRTHDAY")]
        birthday: u64,
        /// Optional lightwalletd gRPC endpoint URL override.
        #[arg(long, env = "LIGHTWALLETD_URL")]
        lightwalletd: Option<String>,
        /// Output file for prepared claims JSON.
        #[arg(long, env = "CLAIMS_OUT", default_value = "claim-prepared.json")]
        claims_out: PathBuf,
    },
    /// Generate claim proofs using custom claim circuit.
    #[cfg(feature = "prove")]
    Prove {
        /// Airdrop configuration file.
        #[arg(
            long,
            env = "CONFIG_FILE",
            value_name = "CONFIG_FILE",
            default_value = "config.json"
        )]
        config: PathBuf,
        /// Input file containing claim inputs.
        #[arg(long, env = "CLAIMS_IN", default_value = "claim-prepared.json")]
        claims_in: PathBuf,
        /// Path to file containing 64-byte seed as hex for deriving spending keys.
        #[arg(long, env = "SEED_FILE", value_name = "SEED_FILE")]
        seed: PathBuf,
        /// Path to proving key file.
        #[arg(
            long,
            env = "PROVING_KEY_FILE",
            value_name = "PROVING_KEY_FILE",
            default_value = "setup-sapling-pk.params"
        )]
        pk: PathBuf,
        /// ZIP-32 account index used to derive Sapling keys from the seed.
        #[arg(long, env = "ACCOUNT_ID", default_value_t = 0_u32)]
        account: u32,
        /// Output file for generated claim proofs.
        #[arg(long, env = "PROOFS_OUT", default_value = "claim-proofs.json")]
        proofs_out: PathBuf,
        /// Output file for local-only claim secrets.
        #[arg(long, env = "SECRETS_OUT", default_value = "claim-proofs-secrets.json")]
        secrets_out: PathBuf,
    },
    /// Sign a Sapling proof bundle into a submission package.
    Sign {
        /// Airdrop configuration file.
        #[arg(
            long,
            env = "CONFIG_FILE",
            value_name = "CONFIG_FILE",
            default_value = "config.json"
        )]
        config: PathBuf,
        /// Proofs file generated by `claim prove`.
        #[arg(long, env = "PROOFS_IN", default_value = "claim-proofs.json")]
        proofs_in: PathBuf,
        /// Local-only secrets file generated by `claim prove`.
        #[arg(long, env = "SECRETS_IN", default_value = "claim-proofs-secrets.json")]
        secrets_in: PathBuf,
        /// Path to file containing 64-byte seed as hex for deriving spending keys.
        #[arg(long, env = "SEED_FILE", value_name = "SEED_FILE")]
        seed: PathBuf,
        /// Message payload file to bind into submission signatures.
        #[arg(
            long,
            env = "MESSAGE_FILE",
            value_name = "MESSAGE_FILE",
            default_value = "claim-message.bin"
        )]
        msg: PathBuf,
        /// ZIP-32 account index used to derive Sapling keys from the seed.
        #[arg(long, env = "ACCOUNT_ID", default_value_t = 0_u32)]
        account: u32,
        /// Output file for signed submission bundle.
        #[arg(long, env = "SUBMISSION_OUT", default_value = "claim-submission.json")]
        submission_out: PathBuf,
    },
}

/// Verify command group.
#[derive(Debug, clap::Subcommand)]
pub enum VerifyCommands {
    /// Recommended end-to-end verification:
    /// `verify proof -> verify signature`.
    Run {
        /// Airdrop configuration file used for proof/signature binding checks.
        #[arg(
            long,
            env = "CONFIG_FILE",
            value_name = "CONFIG_FILE",
            default_value = "config.json"
        )]
        config: PathBuf,
        /// Path to the verifying key file.
        #[arg(
            long,
            env = "VERIFYING_KEY_FILE",
            value_name = "VERIFYING_KEY_FILE",
            default_value = "setup-sapling-vk.params"
        )]
        vk: PathBuf,
        /// Signed submission file generated by `claim sign`.
        #[arg(long, env = "SUBMISSION_IN", default_value = "claim-submission.json")]
        submission_in: PathBuf,
        /// Message payload file used when signing.
        #[arg(
            long,
            env = "MESSAGE_FILE",
            value_name = "MESSAGE_FILE",
            default_value = "claim-message.bin"
        )]
        msg: PathBuf,
    },
    /// Verify claim proofs from a proofs file.
    Proof {
        /// Airdrop configuration file used to bind expected roots and scheme.
        #[arg(
            long,
            env = "CONFIG_FILE",
            value_name = "CONFIG_FILE",
            default_value = "config.json"
        )]
        config: PathBuf,
        /// Path to the verifying key file.
        #[arg(
            long,
            env = "VERIFYING_KEY_FILE",
            value_name = "VERIFYING_KEY_FILE",
            default_value = "setup-sapling-vk.params"
        )]
        vk: PathBuf,
        /// JSON file containing claim proofs.
        #[arg(long, env = "PROOFS_IN", default_value = "claim-proofs.json")]
        proofs_in: PathBuf,
    },
    /// Verify signatures in a signed claim submission.
    Signature {
        /// Airdrop configuration file used to bind expected target-id and pool.
        #[arg(
            long,
            env = "CONFIG_FILE",
            value_name = "CONFIG_FILE",
            default_value = "config.json"
        )]
        config: PathBuf,
        /// Signed submission file generated by `claim sign`.
        #[arg(long, env = "SUBMISSION_IN", default_value = "claim-submission.json")]
        submission_in: PathBuf,
        /// Message payload file used when signing.
        #[arg(
            long,
            env = "MESSAGE_FILE",
            value_name = "MESSAGE_FILE",
            default_value = "claim-message.bin"
        )]
        msg: PathBuf,
    },
}

/// Common arguments for `config build`.
#[derive(Debug, clap::Args)]
pub struct BuildConfigArgs {
    /// Network to use (mainnet or testnet).
    #[arg(long, env = "NETWORK", default_value = "mainnet", value_parser = parse_network)]
    pub network: Network,
    /// Snapshot block height (inclusive).
    #[arg(long, env = "SNAPSHOT_HEIGHT")]
    pub height: u64,
    /// Optional lightwalletd gRPC endpoint URL override.
    #[arg(long, env = "LIGHTWALLETD_URL")]
    pub lightwalletd: Option<String>,
}

impl From<BuildConfigArgs> for CommonConfig {
    fn from(args: BuildConfigArgs) -> Self {
        Self {
            network: args.network,
            snapshot_height: args.height,
            lightwalletd_url: args.lightwalletd,
        }
    }
}

fn parse_network(s: &str) -> Result<Network> {
    match s {
        "mainnet" => Ok(Network::MainNetwork),
        "testnet" => Ok(Network::TestNetwork),
        other => Err(eyre!(
            "Invalid network: {other}. Expected 'mainnet' or 'testnet'."
        )),
    }
}

fn parse_pool_selection(s: &str) -> Result<PoolSelection> {
    match s {
        "sapling" => Ok(PoolSelection::Sapling),
        "orchard" => Ok(PoolSelection::Orchard),
        "both" => Ok(PoolSelection::Both),
        other => Err(eyre!(
            "Invalid pool: {other}. Expected 'sapling', 'orchard', or 'both'."
        )),
    }
}

fn parse_sapling_target_id(s: &str) -> Result<String> {
    ensure!(s.len() == 8, "Sapling target_id must be exactly 8 bytes");
    Ok(s.to_string())
}

fn parse_orchard_target_id(s: &str) -> Result<String> {
    ensure!(s.len() <= 32, "Orchard target_id must be at most 32 bytes");
    Ok(s.to_string())
}

fn parse_value_commitment_scheme(s: &str) -> Result<ValueCommitmentScheme> {
    match s {
        "native" => Ok(ValueCommitmentScheme::Native),
        "sha256" => Ok(ValueCommitmentScheme::Sha256),
        other => Err(eyre!(
            "Invalid value commitment scheme: {other}. Expected 'native' or 'sha256'."
        )),
    }
}

#[cfg(feature = "prove")]
fn parse_setup_scheme(s: &str) -> Result<SaplingSetupScheme> {
    match s {
        "native" => Ok(SaplingSetupScheme::Native),
        "sha256" => Ok(SaplingSetupScheme::Sha256),
        other => Err(eyre!(
            "Invalid setup scheme: {other}. Expected 'native' or 'sha256'."
        )),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::*;

    #[test]
    fn network_parse() {
        let network = parse_network("mainnet").expect("Failed to parse mainnet");
        assert_eq!(network, Network::MainNetwork);
        let network = parse_network("testnet").expect("Failed to parse testnet");
        assert_eq!(network, Network::TestNetwork);
        assert!(parse_network("invalid_network").is_err());
    }

    #[test]
    fn pool_selection_parse() {
        assert!(matches!(
            parse_pool_selection("sapling").expect("sapling should parse"),
            PoolSelection::Sapling
        ));
        assert!(matches!(
            parse_pool_selection("orchard").expect("orchard should parse"),
            PoolSelection::Orchard
        ));
        assert!(matches!(
            parse_pool_selection("both").expect("both should parse"),
            PoolSelection::Both
        ));
        assert!(parse_pool_selection("nope").is_err());
    }

    #[cfg(feature = "prove")]
    #[test]
    fn parse_claim_run_command() {
        let cli = Cli::try_parse_from([
            "zair",
            "claim",
            "run",
            "--seed",
            "seed.txt",
            "--birthday",
            "3663119",
        ]);
        assert!(cli.is_ok());
    }

    #[test]
    fn parse_verify_run_command() {
        let cli = Cli::try_parse_from(["zair", "verify", "run"]);
        assert!(cli.is_ok());
    }
}
