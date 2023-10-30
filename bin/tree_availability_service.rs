//! # Tree Availability Service
//!
//! The tree availability service is able to create an in-memory representation of the World ID
//! merkle tree by syncing the state of the World ID contract `registerIdentities` and `deleteIdentities`
//! function calldata and `TreeChanged` events. Once it syncs the latest state of the state of the tree, it
//! is able to serve inclusion proofs on the `/inclusionProof` endpoint. It also keeps a historical roots list
//! of `tree_history_size` size in order to serve proofs against older tree roots (including the roots of
//! World IDs bridged to other networks).
//!
//! ### Usage
//!
//! #### CLI
//!
//! In order to run the tree availability service you can run the following commands
//!
//! ```bash
//! # Build the binary
//! cargo build --bin tree-availability-service --release
//! # Run the command
//! ./target/release/tree-availability-service  
//!  --tree-depth <TREE_DEPTH>                  
//!  --tree-history-size <TREE_HISTORY_SIZE>    
//!  --dense-prefix-depth <DENSE_PREFIX_DEPTH>  
//!  --address <ADDRESS>                        
//!  --creation-block <CREATION_BLOCK>          
//!  --rpc-endpoint <RPC_ENDPOINT>

use std::sync::Arc;

use clap::Parser;
use ethers::providers::{Http, Provider};
use ethers::types::H160;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tree_availability::TreeAvailabilityService;

/// CLI interface for the Tree Availability Service
#[derive(Parser, Debug)]
#[clap(
    name = "Tree Availability Service",
    about = "The tree availability service periodically calls propagateRoot() on a World ID StateBridge contract."
)]
struct Opts {
    #[clap(long, help = "Depth of the World Tree")]
    tree_depth: usize,
    #[clap(
        long,
        help = "Quantity of recent tree changes to cache. This allows inclusion proof requests to specify a historical root"
    )]
    tree_history_size: usize,
    #[clap(
        short,
        long,
        help = "Depth of merkle tree that should be represented as a densely populated prefix. The remainder of the tree will be represented with pointer-based structures."
    )]
    dense_prefix_depth: usize,
    #[clap(short, long, help = "Address of the World Tree")]
    address: H160,
    #[clap(short, long, help = "Creation block of the World Tree")]
    creation_block: u64,
    #[clap(short, long, help = "Ethereum RPC endpoint")]
    rpc_endpoint: String,
    #[clap(
        short,
        long,
        help = "Port to expose for the tree-availability-service API",
        default_value = "8080"
    )]
    port: u16,
}

#[tokio::main]
pub async fn main() -> eyre::Result<()> {
    let opts = Opts::parse();

    let middleware = Arc::new(Provider::<Http>::try_from(opts.rpc_endpoint)?);
    let handles = TreeAvailabilityService::new(
        opts.tree_depth,
        opts.dense_prefix_depth,
        opts.tree_history_size,
        opts.address,
        opts.creation_block,
        middleware,
    )
    .serve(opts.port)
    .await;

    let mut handles = handles.into_iter().collect::<FuturesUnordered<_>>();
    while let Some(result) = handles.next().await {
        result??;
    }

    Ok(())
}
