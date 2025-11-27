//! Light Wallet API for interacting with Zcash light wallets.
#[allow(missing_docs)]
mod rpc {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

pub use rpc::*;
