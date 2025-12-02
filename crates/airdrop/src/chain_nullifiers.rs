use std::path::PathBuf;
use std::pin::Pin;

use futures::{Stream, StreamExt as _};
use non_membership_proofs::nullifier_source::file::FileSource;
use non_membership_proofs::nullifier_source::light_walletd::LightWalletd;
use non_membership_proofs::nullifier_source::{NullifierSource, PoolNullifier};

use crate::CommonArgs;
use crate::cli::Source;

/// Stream of nullifiers with unified error type
type NullifierStream = Pin<Box<dyn Stream<Item = eyre::Result<PoolNullifier>> + Send>>;

/// Get a stream of nullifiers based on the configuration
pub(crate) async fn get_nullifiers(config: &CommonArgs) -> eyre::Result<NullifierStream> {
    match config.source.clone().try_into()? {
        Source::Lightwalletd { url } => {
            let source =
                LightWalletd::connect(&url, *config.snapshot.start(), *config.snapshot.end()).await?;
            Ok(Box::pin(
                source
                    .into_nullifiers_stream()
                    .map(|r| r.map_err(Into::into)),
            ))
        }
        Source::File { orchard, sapling } => {
            let source = FileSource::new(PathBuf::from(sapling), PathBuf::from(orchard));
            Ok(Box::pin(
                source
                    .into_nullifiers_stream()
                    .map(|r| r.map_err(Into::into)),
            ))
        }
    }
}
