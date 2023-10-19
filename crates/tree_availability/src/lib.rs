pub mod abi;
pub mod error;
pub mod server;
pub mod tree;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use error::TreeAvailabilityError;
use ethers::contract::EthEvent;
use ethers::providers::{Middleware, StreamExt};
use ethers::types::{Filter, Log, H160};
use semaphore::lazy_merkle_tree::Canonical;
use tokio::task::JoinHandle;
use tree::{Hash, PoseidonTree, WorldTree};

use crate::abi::TreeChangedFilter;
use crate::server::inclusion_proof;

//TODO: update the default port
const DEFAULT_PORT: u16 = 8080;
//TODO: Should use stream instead of watch

pub struct TreeAvailabilityService<M: Middleware + 'static> {
    pub world_tree: Arc<WorldTree<M>>,
}

impl<M: Middleware> TreeAvailabilityService<M> {
    pub fn new(
        tree_depth: usize,
        dense_prefix_depth: usize,
        tree_history_size: usize,
        world_tree_address: H160,
        world_tree_creation_block: u64,
        middleware: Arc<M>,
    ) -> Self {
        dbg!("Creating new tree");

        let tree = PoseidonTree::<Canonical>::new_with_dense_prefix(
            tree_depth,
            dense_prefix_depth,
            &Hash::ZERO,
        );

        dbg!("Initializing new world tree");

        let world_tree = Arc::new(WorldTree::new(
            tree,
            tree_history_size,
            world_tree_address,
            world_tree_creation_block,
            middleware,
        ));

        Self { world_tree }
    }

    pub async fn spawn(
        &self,
    ) -> Vec<JoinHandle<Result<(), TreeAvailabilityError<M>>>> {
        let mut handles = vec![];

        let (mut rx, updates_handle) = self.world_tree.listen_for_updates();
        // Spawn a thread to listen to tree changed events with a buffer
        handles.push(updates_handle);

        dbg!("Syncing world tree to head");
        // Sync the world tree to the chain head
        self.world_tree
            .sync_to_head()
            .await
            .expect("TODO: error handling");

        let world_tree = self.world_tree.clone();

        handles.push(tokio::spawn(async move {
            while let Some(log) = rx.recv().await {
                world_tree.sync_from_log(log).await?;
            }

            Ok(())
        }));

        handles
    }

    pub async fn serve(
        self,
        port: Option<u16>,
    ) -> Vec<JoinHandle<Result<(), TreeAvailabilityError<M>>>> {
        let mut handles = vec![];

        dbg!("Spawning tree availability service");
        // Spawn a new task to keep the world tree synced to the chain head
        let world_tree_handles = self.spawn().await;
        handles.extend(world_tree_handles);

        dbg!("Initializing router");

        // Initialize a new router and spawn the server
        let router = axum::Router::new()
            .route("/inclusionProof", axum::routing::post(inclusion_proof))
            // .route("/verifyProof", axum::routing::post(verify_proof))
            .with_state(self.world_tree.clone());

        let address = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port.unwrap_or_else(|| DEFAULT_PORT),
        );

        dbg!("Spawning server");

        let server_handle = tokio::spawn(async move {
            axum::Server::bind(&address)
                .serve(router.into_make_service())
                .await
                .map_err(TreeAvailabilityError::HyperError)?;
            // .with_graceful_shutdown(await_shutdown());

            Ok(())
        });

        handles.push(server_handle);

        handles
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use ethers::providers::{Provider, Ws};
    use ethers::types::H160;

    use crate::TreeAvailabilityService;

    const DEFAULT_TREE_HISTORY_SIZE: usize = 10;

    //TODO: set world tree address as const for tests

    // #[tokio::test]
    // async fn test_spawn_tree_availability_service() -> eyre::Result<()> {

    // }
}
