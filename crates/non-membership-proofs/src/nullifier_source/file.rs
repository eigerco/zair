//! Read nullifiers from local files
//! This is used for testing and local setups
//! The expected file format is a sequence of 32-byte nullifiers

use std::io;
use std::path::PathBuf;
use std::pin::Pin;

use async_stream::try_stream;
use futures_core::Stream;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};

use crate::nullifier_source::{NullifierSource, Pool, PoolNullifier};

/// Read nullifiers from local files
pub struct FileSource {
    sapling_path: PathBuf,
    orchard_path: PathBuf,
}

impl FileSource {
    /// Create a new FileSource with the given file paths
    pub fn new(sapling_path: PathBuf, orchard_path: PathBuf) -> Self {
        Self {
            sapling_path,
            orchard_path,
        }
    }
}

impl NullifierSource for FileSource {
    type Error = io::Error;
    type Stream = Pin<Box<dyn Stream<Item = Result<PoolNullifier, Self::Error>> + Send>>;

    fn into_nullifiers_stream(self) -> Self::Stream {
        Box::pin(try_stream! {
            let mut buf = vec![0u8; 32 * (1024)]; // Read 32 KiB at a time (1024 nullifiers)

            for (file, pool) in [
                (self.sapling_path, Pool::Sapling),
                (self.orchard_path, Pool::Orchard),
            ] {
                let file = File::open(file).await?;
                let mut reader = BufReader::new(file);

                loop {
                    let n = reader.read(&mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    for chunk in buf[..n].chunks(32) {
                        if chunk.len() == 32 {
                            let mut nullifier = [0u8; 32];
                            nullifier.copy_from_slice(chunk);
                            yield PoolNullifier {
                                pool,
                                nullifier,
                            };
                        }
                    }
                }
            }
        })
    }
}
