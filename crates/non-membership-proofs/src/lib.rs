//! Non-membership proofs library

// TODO: remove hardcoded values from this file

pub mod nullifier_source;
pub mod utils;

use std::path::Path;

use futures::{Stream, TryStreamExt as _};
use nullifier_source::{Nullifier, Pool, PoolNullifier};
use rs_merkle::{Hasher, MerkleTree};
use tokio::fs::File;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _, BufReader, BufWriter};

/// Collect stream into separate pools
///
/// TODO: use Vec::capacity
pub async fn partition_by_pool<S, E>(stream: S) -> Result<(Vec<Nullifier>, Vec<Nullifier>), E>
where
    S: Stream<Item = Result<PoolNullifier, E>>,
{
    let mut sapling = Vec::new();
    let mut orchard = Vec::new();

    tokio::pin!(stream);
    while let Some(nullifier) = stream.try_next().await? {
        match nullifier.pool {
            Pool::Sapling => sapling.push(nullifier.nullifier),
            Pool::Orchard => orchard.push(nullifier.nullifier),
        }
    }

    Ok((sapling, orchard))
}

/// Build a Merkle tree from the given nullifiers to produce non-membership proofs
///
/// Algorithm:
/// 1. Sort the nullifiers
/// 2. Concatenate each consecutive nullifiers to store ranges of nullifiers in leaf nodes.
/// Merge: [nf1, nf2, nf3, nf4] -> [(nf1, nf2), (nf2, nf3)]
/// 3. Hash each leaf node
pub fn build_merkle_tree<H: Hasher>(nullifiers: &mut [Nullifier]) -> MerkleTree<H> {
    nullifiers.sort_unstable();

    let leaves = nullifiers
        .windows(2)
        .map(|window| {
            let mut merged = [0u8; 64];
            merged[..32].copy_from_slice(&window[0]);
            merged[32..].copy_from_slice(&window[1]);
            merged
        })
        .map(|leaf| H::hash(&leaf))
        .collect::<Vec<_>>();

    MerkleTree::from_leaves(&leaves)
}

/// Write leaf notes to binary file without intermediate allocation
pub async fn write_raw_nullifiers<P>(notes: &[[u8; 32]], path: P) -> std::io::Result<()>
where
    P: AsRef<Path>,
{
    let file = File::open(path).await?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, file);

    writer.write_all(bytemuck::cast_slice(notes)).await?;

    Ok(())
}

/// Read leaf notes from binary file without intermediate allocation
pub async fn read_raw_nullifiers<P>(path: P) -> std::io::Result<Vec<[u8; 32]>>
where
    P: AsRef<Path>,
{
    let file = File::open(path).await?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await?;
    let notes: Vec<[u8; 32]> = bytemuck::cast_slice(&buf).to_vec();

    Ok(notes)
}
