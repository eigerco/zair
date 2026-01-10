use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use non_membership_proofs::utils::SanitiseNullifiers;
use non_membership_proofs::{NonMembershipTree, partition_by_pool, write_nullifiers};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::BufWriter;
use tracing::{info, instrument, warn};

use crate::cli::CommonArgs;
use crate::{BUF_SIZE, chain_nullifiers};

/// Configuration for an airdrop, including snapshot range and Merkle roots and the hiding factors
/// for each Zcash pool.
#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub struct AirdropConfiguration {
    /// The inclusive range of block heights for the snapshot.
    pub snapshot_range: RangeInclusive<u64>,
    /// The Merkle root for the Sapling shielded addresses.
    pub sapling_merkle_root: Option<String>,
    /// The Merkle root for the Orchard shielded addresses.
    pub orchard_merkle_root: Option<String>,
    /// Hiding factor for nullifiers
    #[serde(default)]
    pub hiding_factor: HidingFactor,
}

/// Hiding factor for hiding-nullifier derivation
#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
pub struct HidingFactor {
    /// Hiding factor for Sapling hiding-nullifiers
    pub sapling: SaplingHidingFactor,
    /// Hiding factor for Orchard hiding-nullifiers
    pub orchard: OrchardHidingFactor,
}

/// Sapling hiding factor
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default, clap::Args)]
pub struct SaplingHidingFactor {
    /// Personalization bytes, are used to derive the hiding sapling nullifier
    #[arg(long)]
    pub personalization: Vec<u8>,
}

/// Orchard hiding factor
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default, clap::Args)]
pub struct OrchardHidingFactor {
    /// Domain separator for the hiding orchard nullifier
    #[arg(long)]
    pub domain: String,
    /// Tag bytes, are used to derive the hiding orchard nullifier
    #[arg(long)]
    pub tag: Vec<u8>,
}

impl<'a> From<&'a SaplingHidingFactor>
    for non_membership_proofs::user_nullifiers::SaplingHidingFactor<'a>
{
    fn from(owned: &'a SaplingHidingFactor) -> Self {
        Self {
            personalization: &owned.personalization,
        }
    }
}

impl<'a> From<&'a OrchardHidingFactor>
    for non_membership_proofs::user_nullifiers::OrchardHidingFactor<'a>
{
    fn from(owned: &'a OrchardHidingFactor) -> Self {
        Self {
            domain: &owned.domain,
            tag: &owned.tag,
        }
    }
}

impl AirdropConfiguration {
    pub const fn new(
        snapshot_range: RangeInclusive<u64>,
        sapling_merkle_root: Option<String>,
        orchard_merkle_root: Option<String>,
        hiding_factor: HidingFactor,
    ) -> Self {
        Self {
            snapshot_range,
            sapling_merkle_root,
            orchard_merkle_root,
            hiding_factor,
        }
    }

    pub async fn export_config(&self, destination: impl AsRef<Path>) -> eyre::Result<()> {
        let config_json = serde_json::to_string_pretty(self)?;
        tokio::fs::write(destination, config_json).await?;
        Ok(())
    }
}

#[instrument(skip_all, fields(
    snapshot = %format!("{}..={}", config.snapshot.start(), config.snapshot.end())
))]
pub async fn build_airdrop_configuration(
    config: CommonArgs,
    configuration_output_file: PathBuf,
    sapling_snapshot_nullifiers: PathBuf,
    orchard_snapshot_nullifiers: PathBuf,
    hiding_factor: HidingFactor,
) -> eyre::Result<()> {
    info!("Fetching nullifiers");
    let stream = chain_nullifiers::get_nullifiers(&config).await?;
    let (sapling_nullifiers, orchard_nullifiers) = partition_by_pool(stream).await?;

    let sapling_handle = tokio::spawn(process_pool(
        "sapling",
        SanitiseNullifiers::new(sapling_nullifiers),
        sapling_snapshot_nullifiers,
    ));
    let orchard_handle = tokio::spawn(process_pool(
        "orchard",
        SanitiseNullifiers::new(orchard_nullifiers),
        orchard_snapshot_nullifiers,
    ));

    let (sapling_root, orchard_root) = tokio::try_join!(sapling_handle, orchard_handle)?;
    let sapling_root = sapling_root?;
    let orchard_root = orchard_root?;

    AirdropConfiguration::new(config.snapshot, sapling_root, orchard_root, hiding_factor)
        .export_config(&configuration_output_file)
        .await?;

    info!(file = ?configuration_output_file, "Exported configuration");
    Ok(())
}

#[instrument(skip_all, fields(pool = %pool, store = %store.display()))]
async fn process_pool(
    pool: &str,
    nullifiers: SanitiseNullifiers,
    store: PathBuf,
) -> eyre::Result<Option<String>> {
    if nullifiers.is_empty() {
        warn!(pool, "No nullifiers collected");
        return Ok(None);
    }

    info!(count = nullifiers.len(), "Collected nullifiers");

    let file = File::create(&store).await?;
    let mut writer = BufWriter::with_capacity(BUF_SIZE, file);
    write_nullifiers(&nullifiers, &mut writer).await?;
    info!(file = ?store, pool, "Saved nullifiers");

    let merkle_tree =
        tokio::task::spawn_blocking(move || NonMembershipTree::from_nullifiers(&nullifiers))
            .await??;

    let root = merkle_tree.root();
    let root_hex = hex::encode(root.to_bytes());
    info!(pool, root = %root_hex, "Built merkle tree");

    Ok(Some(root_hex))
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;
    use tokio::fs::File;
    use tokio::io::AsyncReadExt;

    use super::*;

    #[test]
    fn deserialize_json_format() {
        // Documents the expected JSON format for consumers
        let json = r#"{
          "snapshot_range": { "start": 100, "end": 200 },
          "sapling_merkle_root": "abc",
          "orchard_merkle_root": null
        }"#;

        let json_config: AirdropConfiguration =
            serde_json::from_str(json).expect("Failed to deserialize JSON");

        let expected_config = AirdropConfiguration::new(
            100..=200,
            Some("abc".to_string()),
            None,
            HidingFactor::default(),
        );
        assert_eq!(json_config.snapshot_range, expected_config.snapshot_range);
    }

    #[tokio::test]
    async fn export_config() {
        let config = AirdropConfiguration::new(
            100..=200,
            Some("sapling".to_string()),
            Some("orchard".to_string()),
            HidingFactor::default(),
        );
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let path = temp_file.path();

        config
            .export_config(path)
            .await
            .expect("Failed to export config");

        let mut file = File::open(path)
            .await
            .expect("Failed to open exported config");
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .await
            .expect("Failed to read exported config");

        let loaded: AirdropConfiguration =
            serde_json::from_str(&contents).expect("Failed to deserialize exported config");
        assert_eq!(config, loaded);
    }

    #[test]
    fn sanity_check_conversions() {
        let sapling_hiding = SaplingHidingFactor {
            personalization: vec![1, 2, 3],
        };
        let orchard_hiding = OrchardHidingFactor {
            domain: "domain".to_string(),
            tag: vec![4, 5, 6],
        };

        let sapling_converted: non_membership_proofs::user_nullifiers::SaplingHidingFactor =
            (&sapling_hiding).into();
        assert_eq!(sapling_converted.personalization, &[1, 2, 3]);

        let orchard_converted: non_membership_proofs::user_nullifiers::OrchardHidingFactor =
            (&orchard_hiding).into();
        assert_eq!(orchard_converted.domain, "domain");
        assert_eq!(orchard_converted.tag, &[4, 5, 6]);
    }
}
