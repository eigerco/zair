//! Non-membership proofs library

pub mod chain_nullifiers;
pub mod source;
pub mod user_nullifiers;
pub mod utils;

use std::path::Path;

use chain_nullifiers::PoolNullifier;
use futures::{Stream, TryStreamExt as _};
use rayon::iter::ParallelIterator as _;
use rayon::slice::ParallelSlice as _;
use rs_merkle::{Hasher, MerkleTree};
use thiserror::Error;
use tokio::fs::File;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _, BufReader, BufWriter};

/// Buffer size for file I/O
const BUF_SIZE: usize = 1024 * 1024;

/// Size of a nullifier in bytes
const NULLIFIER_SIZE: usize = 32;

/// A representation of Nullifiers
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

/// Collect stream into separate vectors, by pool.
///
/// # Errors
///
/// Returns an error if the stream returns an error.
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

/// Errors that can occur when building a Merkle tree for non-membership proofs
#[derive(Error, Debug)]
pub enum MerkleTreeError {
    /// Nullifiers are not sorted.
    /// Nullifiers must be sorted to build the Merkle tree for non-membership proofs.
    #[error(
        "Nullifiers are not sorted. Nullifiers must be sorted to build the Merkle tree for non-membership proofs."
    )]
    NotSorted,
}

/// Builds a Merkle tree from a sorted slice of nullifiers for non-membership proofs.
///
/// Note: This function may be CPU-intensive for large slices. If used in an async context, consider
/// offloading to a blocking thread.
///
/// # Arguments
///
/// * `nullifiers` - A slice of nullifiers, which must be sorted in ascending order.
///
/// # Returns
///
/// Returns a `MerkleTree` constructed from the nullifiers, or an error if the input is not sorted.
///
/// # Errors
///
/// Returns [`MerkleTreeError::NotSorted`] if the input slice is not sorted in ascending order.
///
/// # Algorithm
///
/// - Adds a "front" leaf node representing the range from 0 to the first nullifier.
/// - Adds leaf nodes for each consecutive pair of nullifiers.
/// - Adds a "back" leaf node representing the range from the last nullifier to 0xFF..FF.
/// - Hashes each leaf node and constructs the Merkle tree from these hashes.
#[allow(
    clippy::panic_in_result_fn,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    clippy::missing_asserts_for_indexing,
    reason = "Panics are impossible: we check is_empty() before .expect(), and windows(2) guarantees 2 elements"
)]
pub fn build_merkle_tree<H>(nullifiers: &[Nullifier]) -> Result<MerkleTree<H>, MerkleTreeError>
where
    H: Hasher,
    H::Hash: Send,
{
    if nullifiers.is_empty() {
        return Ok(MerkleTree::new());
    }

    if !nullifiers.is_sorted() {
        return Err(MerkleTreeError::NotSorted);
    }

    // Safe: we already checked nullifiers is not empty above
    let first = nullifiers
        .first()
        .expect("Nullifiers array is not empty, and this should always have a value");
    let last = nullifiers
        .last()
        .expect("Nullifiers array is not empty, and this should always have a value");

    let front = H::hash(&build_leaf(&[0_u8; NULLIFIER_SIZE], first));
    let back = H::hash(&build_leaf(last, &[0xFF; NULLIFIER_SIZE]));

    // Pre-allocate: 1 front + (n-1) windows + 1 back = n + 1
    let mut leaves = Vec::with_capacity(nullifiers.len().saturating_add(1));

    leaves.push(front);
    leaves.extend(
        nullifiers
            .par_windows(2)
            .map(|w| H::hash(&build_leaf(&w[0], &w[1])))
            .collect::<Vec<_>>(),
    );
    leaves.push(back);

    Ok(MerkleTree::from_leaves(&leaves))
}

/// Build a leaf node from two nullifiers
#[must_use]
pub fn build_leaf(nf1: &Nullifier, nf2: &Nullifier) -> [u8; 2 * NULLIFIER_SIZE] {
    let mut leaf = [0_u8; 2 * NULLIFIER_SIZE];
    leaf[..NULLIFIER_SIZE].copy_from_slice(nf1);
    leaf[NULLIFIER_SIZE..].copy_from_slice(nf2);
    leaf
}

/// Write nullifiers to binary file without intermediate allocation
///
/// # Errors
/// If writing to the file fails
pub async fn write_raw_nullifiers<P>(nullifiers: &[Nullifier], path: P) -> std::io::Result<()>
where
    P: AsRef<Path>,
{
    let file = File::create(path).await?;
    let mut writer = BufWriter::with_capacity(BUF_SIZE, file);

    writer.write_all(bytemuck::cast_slice(nullifiers)).await?;
    writer.flush().await?;

    Ok(())
}

/// Read nullifiers from binary file without intermediate allocation
///
/// # Errors
/// If reading from the file fails
pub async fn read_raw_nullifiers<P>(path: P) -> std::io::Result<Vec<Nullifier>>
where
    P: AsRef<Path>,
{
    let file = File::open(path).await?;
    let mut reader = BufReader::with_capacity(BUF_SIZE, file);

    let mut buf = Vec::with_capacity(BUF_SIZE);
    reader.read_to_end(&mut buf).await?;
    let nullifiers: Vec<Nullifier> = bytemuck::cast_slice(&buf).to_vec();

    Ok(nullifiers)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "Tests")]

    use rs_merkle::algorithms::Sha256;

    use super::*;

    /// Helper macro to create a nullifier with a specific last byte.
    macro_rules! nf {
        ($v:expr) => {{
            let mut arr = [0_u8; 32];
            arr[31] = $v;
            arr
        }};
    }

    /// Helper macro to create a sorted vector of nullifiers.
    macro_rules! nfs {
        ($($v:expr),* $(,)?) => {{
            let mut v = vec![$( nf!($v) ),*];
            v.sort();
            v
        }};
    }

    mod build_merkle_tree_tests {
        use super::*;

        #[test]
        fn empty_nullifiers_returns_empty_tree() {
            let nullifiers: Vec<Nullifier> = vec![];
            let tree = build_merkle_tree::<Sha256>(&nullifiers).unwrap();
            assert_eq!(tree.leaves_len(), 0);
            assert!(tree.root().is_none());
        }

        #[test]
        fn single_nullifier_creates_two_leaves() {
            let nullifiers = nfs![50];
            let tree = build_merkle_tree::<Sha256>(&nullifiers).unwrap();
            // 1 nullifier => front leaf + back leaf = 2 leaves
            assert_eq!(tree.leaves_len(), 2);
            assert!(tree.root().is_some());
        }

        #[test]
        fn multiple_nullifiers_creates_correct_leaf_count() {
            let nullifiers = nfs![10, 20, 30, 40];
            let tree = build_merkle_tree::<Sha256>(&nullifiers).unwrap();
            // n nullifiers => n + 1 leaves (front + n-1 windows + back)
            assert_eq!(tree.leaves_len(), 5);
        }

        #[test]
        fn unsorted_nullifiers_returns_error() {
            let unsorted = vec![nf!(50), nf!(10)]; // intentionally unsorted
            let result = build_merkle_tree::<Sha256>(&unsorted);
            assert!(matches!(result, Err(MerkleTreeError::NotSorted)));
        }

        #[test]
        fn sorted_nullifiers_succeeds() {
            let sorted = nfs![10, 20, 30];
            let result = build_merkle_tree::<Sha256>(&sorted);
            assert!(result.is_ok());
        }

        #[test]
        fn tree_is_deterministic() {
            let nullifiers = nfs![10, 20, 30, 40, 50];
            let tree1 = build_merkle_tree::<Sha256>(&nullifiers).unwrap();
            let tree2 = build_merkle_tree::<Sha256>(&nullifiers).unwrap();
            assert_eq!(tree1.root(), tree2.root());
        }

        #[test]
        fn different_nullifiers_produce_different_roots() {
            let nullifiers1 = nfs![10, 20, 30];
            let nullifiers2 = nfs![10, 20, 31];
            let tree1 = build_merkle_tree::<Sha256>(&nullifiers1).unwrap();
            let tree2 = build_merkle_tree::<Sha256>(&nullifiers2).unwrap();
            assert_ne!(tree1.root(), tree2.root());
        }
    }

    mod build_leaf_tests {
        use super::*;

        #[test]
        fn build_leaf_concatenates_nullifiers() {
            let nf1 = nf!(10);
            let nf2 = nf!(20);
            let leaf = build_leaf(&nf1, &nf2);

            assert_eq!(leaf.len(), 64);
            assert_eq!(&leaf[..32], &nf1);
            assert_eq!(&leaf[32..], &nf2);
        }

        #[test]
        fn build_leaf_is_deterministic() {
            let nf1 = nf!(10);
            let nf2 = nf!(20);
            let leaf1 = build_leaf(&nf1, &nf2);
            let leaf2 = build_leaf(&nf1, &nf2);
            assert_eq!(leaf1, leaf2);
        }

        #[test]
        fn build_leaf_order_matters() {
            let nf1 = nf!(10);
            let nf2 = nf!(20);
            let leaf1 = build_leaf(&nf1, &nf2);
            let leaf2 = build_leaf(&nf2, &nf1);
            assert_ne!(leaf1, leaf2);
        }
    }

    mod file_io_tests {
        use super::*;
        use tempfile::tempdir;

        #[tokio::test]
        async fn write_and_read_nullifiers_roundtrip() {
            let dir = tempdir().unwrap();
            let path = dir.path().join("nullifiers.bin");

            let nullifiers = nfs![10, 20, 30, 40, 50];
            write_raw_nullifiers(&nullifiers, &path).await.unwrap();

            let read_back = read_raw_nullifiers(&path).await.unwrap();
            assert_eq!(nullifiers, read_back);
        }

        #[tokio::test]
        async fn write_and_read_empty_nullifiers() {
            let dir = tempdir().unwrap();
            let path = dir.path().join("empty.bin");

            let nullifiers: Vec<Nullifier> = vec![];
            write_raw_nullifiers(&nullifiers, &path).await.unwrap();

            let read_back = read_raw_nullifiers(&path).await.unwrap();
            assert!(read_back.is_empty());
        }

        #[tokio::test]
        async fn write_and_read_single_nullifier() {
            let dir = tempdir().unwrap();
            let path = dir.path().join("single.bin");

            let nullifiers = vec![nf!(42)];
            write_raw_nullifiers(&nullifiers, &path).await.unwrap();

            let read_back = read_raw_nullifiers(&path).await.unwrap();
            assert_eq!(nullifiers, read_back);
        }

        #[tokio::test]
        async fn read_nonexistent_file_returns_error() {
            let result = read_raw_nullifiers("/nonexistent/path/nullifiers.bin").await;
            assert!(result.is_err());
        }
    }
}
