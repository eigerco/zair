//! Read nullifiers from a lightwalletd via gRPC

use std::pin::Pin;

use async_stream::try_stream;
use futures_core::Stream;
use light_wallet_api::compact_tx_streamer_client::CompactTxStreamerClient;
use light_wallet_api::{BlockId, BlockRange};
use tonic::transport::Channel;

use crate::nullifier_source::{Nullifier, NullifierSource, Pool, PoolNullifier};

/// Read nullifiers from a lightwalletd via gRPC
pub struct LightWalletd {
    client: CompactTxStreamerClient<Channel>,
    start_height: u64,
    end_height: u64,
}

/// Errors that can occur when interucting with lightwalletd
#[derive(Debug, thiserror::Error)]
pub enum LightWalletdError {
    /// gRPC error from lightwalletd
    #[error("gRPC: {0}")]
    Grpc(#[from] tonic::Status),
    /// Transport error connecting to lightwalletd
    #[error("Transport: {0}")]
    Transport(#[from] tonic::transport::Error),
    /// Invalid nullifier length
    #[error("Invalid nullifier length: expected 32, got {0}")]
    InvalidLength(usize),
}

impl LightWalletd {
    /// Connect to a lightwalletd endpoint
    ///
    /// Prerequisite:
    /// rustls::crypto::ring::default_provider().install_default() needs to be called before this
    /// function is called.
    pub async fn connect(
        endpoint: &str,
        start_height: u64, // TODO: remove the heights from here
        end_height: u64,
    ) -> Result<Self, LightWalletdError> {
        let client = CompactTxStreamerClient::connect(endpoint.to_string()).await?;

        Ok(Self {
            client,
            start_height,
            end_height,
        })
    }
}

impl NullifierSource for LightWalletd {
    type Error = LightWalletdError;
    type Stream = Pin<Box<dyn Stream<Item = Result<PoolNullifier, Self::Error>> + Send>>;

    fn into_nullifiers_stream(self) -> Self::Stream {
        let request = BlockRange {
            start: Some(BlockId {
                height: self.start_height,
                hash: vec![],
            }),
            end: Some(BlockId {
                height: self.end_height,
                hash: vec![],
            }),
            pool_types: vec![],
        };

        let mut client = self.client;

        Box::pin(try_stream! {
            let mut stream = client
                .get_block_range_nullifiers(request)
                .await?
                .into_inner();

            while let Some(block) = stream.message().await? {
                for tx in block.vtx {
                    // Sapling nullifiers
                    for spend in tx.spends {
                        let nullifier: Nullifier = spend.nf
                            .try_into()
                            .map_err(|v: Vec<u8>| LightWalletdError::InvalidLength(v.len()))?;

                        yield PoolNullifier {
                            pool: Pool::Sapling,
                            nullifier,
                        };
                    }

                    // Orchard nullifiers
                    for action in tx.actions {
                        let nullifier: Nullifier = action.nullifier
                            .try_into()
                            .map_err(|v: Vec<u8>| LightWalletdError::InvalidLength(v.len()))?;

                        yield PoolNullifier {
                            pool: Pool::Orchard,
                            nullifier,
                        };
                    }
                }
            }
        })
    }
}
