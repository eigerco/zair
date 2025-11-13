//! Helper functions to derive Orchard and Sapling Full Viewing Keys from a BIP-39 mnemonic phrase.

use bip39::Language;
use clap_derive::ValueEnum;
use eyre::{Result, WrapErr as _};
use orchard::keys::FullViewingKey as OrchardFvk;
use sapling_crypto::keys::FullViewingKey as SaplingFvk;
use zcash_primitives::zip32::AccountId;

/// Enum representing the Zcash pool types for which Full Viewing Keys can be derived
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Pool {
    /// Sapling pool
    Sapling,
    /// Orchard pool
    Orchard,
    /// Both pools, Sapling and Orchard
    Both,
}

/// Reads the mnemonic from the `ZCASH_MNEMONIC` environment variable, or prompts the user to enter
/// it securely if the variable is not set.
///
/// # Errors
/// Returns an `std::io::Error` if there was an error reading the input.
///
/// # Returns
/// A `Result` containing the mnemonic as a `String` if successful, or an `std::io::Error` if
/// there was an error reading the input.
pub fn read_mnemonic_secure() -> std::io::Result<String> {
    if let Ok(mnemonic) = std::env::var("ZCASH_MNEMONIC") {
        return Ok(mnemonic);
    }

    rpassword::prompt_password("Enter mnemonic: ").map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!("Failed to read mnemonic from terminal: {e}"),
        )
    })
}

/// Enum representing the Zcash coin type for different networks
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum CoinType {
    /// Zcash Mainnet
    Mainnet,
    /// Zcash Testnet
    Testnet,
    /// Zcash Regtest
    Regtest,
}

impl CoinType {
    const fn to_u32(self) -> u32 {
        match self {
            Self::Mainnet => zcash_primitives::constants::mainnet::COIN_TYPE,
            Self::Testnet => zcash_primitives::constants::testnet::COIN_TYPE,
            Self::Regtest => zcash_primitives::constants::regtest::COIN_TYPE,
        }
    }
}

/// Derives Orchard and Sapling Full Viewing Keys from a BIP-39 mnemonic phrase
///
/// # Arguments
/// - `phrase`: The BIP-39 mnemonic phrase as a string slice
/// - `coin_type`: The Zcash coin type (Mainnet, Testnet, Regtest)
///
/// # Returns
/// A Result containing a tuple of (`OrchardFvk`, `SaplingFvk`)
///
/// # Errors
/// Returns an error if the mnemonic phrase is invalid or key derivation fails
pub fn mnemonic_to_fvks(phrase: &str, coin_type: CoinType) -> Result<(OrchardFvk, SaplingFvk)> {
    let m = bip39::Mnemonic::parse_in_normalized(Language::English, phrase)
        .wrap_err("Failed to parse BIP-39 mnemonic phrase")?;
    let seed = m.to_seed("");

    let orchard_fvk =
        orchard_fvk(&seed, coin_type).wrap_err("Failed to derive Orchard Full Viewing Key")?;
    let sapling_fvk = sapling_fvk(&seed, coin_type);

    Ok((orchard_fvk, sapling_fvk))
}

fn orchard_fvk(seed: &[u8; 64], coin_type: CoinType) -> Result<OrchardFvk> {
    use orchard::keys::SpendingKey;
    let orchard_spk = SpendingKey::from_zip32_seed(seed, coin_type.to_u32(), AccountId::ZERO) // TODO:handle AccountId if needed
        .map_err(|e| eyre::eyre!(e))
        .wrap_err_with(|| {
            format!(
                "Failed to derive Orchard spending key from ZIP-32 seed for coin type {coin_type:?}"
            )
        })?;
    let orchard_fvk = OrchardFvk::from(&orchard_spk);

    Ok(orchard_fvk)
}

fn sapling_fvk(seed: &[u8; 64], coin_type: CoinType) -> SaplingFvk {
    use sapling_crypto::zip32::ExtendedSpendingKey;
    use zip32::ChildIndex;

    let master = ExtendedSpendingKey::master(seed);
    let purpose = master.derive_child(ChildIndex::hardened(32)); // TODO: understand why 32 is used here
    let coin = purpose.derive_child(ChildIndex::hardened(coin_type.to_u32()));
    let sapling_ext_spk = coin.derive_child(ChildIndex::hardened(0));
    let sapling_ext_fvk = sapling_ext_spk.to_diversifiable_full_viewing_key();

    sapling_ext_fvk.fvk().clone()
}
