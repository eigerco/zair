//! This module defines the NullifierSource trait and its implementations.
//! NullifierSource provides a streaming interface to read nullifiers from various sources.

use futures_core::Stream;

pub mod file;
pub mod light_walletd;

/// A reprecentation of Nullifiers
///
/// Nullifiers in Zcash Orchard and Sapling pools are both 32 bytes long.
pub type Nullifier = [u8; 32];

/// Zcash pools
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pool {
    /// Sapling pool
    Sapling,
    /// Orchard pool
    Orchard,
}

/// A nullifier tagged with its pool
#[derive(Debug, Clone)]
pub struct PoolNullifier {
    /// The pool the nullifier belongs to
    pub pool: Pool,
    /// The nullifier itself
    pub nullifier: Nullifier,
}

/// This trait defines how to read nullifiers
///
/// The streaming interface is used to be inline with the lightwalletd gRPC interface.
pub trait NullifierSource: Sized {
    /// The error type for this source
    type Error: std::error::Error + Send + 'static;

    /// The concrete stream type returned by this source
    type Stream: Stream<Item = Result<PoolNullifier, Self::Error>> + Send;

    /// Consume self and return a stream of all nullifiers (both Sapling and Orchard)
    fn into_nullifiers_stream(self) -> Self::Stream;
}
