//1st terminal cargo run
//2nd terminal HTTP_PORT=3002 P2P_PORT=5002 PEERS=127.0.0.1:5001 cargo run
//3rd terminal HTTP_PORT=3003 P2P_PORT=5003 PEERS=127.0.0.1:5001,127.0.0.1:5002 cargo run

use crate::config::{CHILDREN, GETDATA_LIMIT, INVMESSAGE_LIMIT};
use crate::treechain::block::Block;
use crate::treechain::treechain::{PQP, TreeChain};
use TreeChainTheorey::treechain;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};

const MESSAGE_TYPE_CONNECTION_INFO: &str = "CONNECTION_INFO";
const MESSAGE_TYPE_GETBLOCKS: &str = "GETBLOCKS";
const MESSAGE_TYPE_INVMESSAGE: &str = "INVMESSAGE";
const MESSAGE_TYPE_GETDATA: &str = "GETDATA";
const MESSAGE_TYPE_BLOCK: &str = "BLOCK";
const MESSAGE_TYPE_MINEDBLOCK: &str = "MINED_BLOCK";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvEntry {
    pub queue_index: u32,
    pub block_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvMessage {
    pub inventories: Vec<InvEntry>,
}

pub struct SyncState {
    pub pending: usize,             // blocks left to receive in the current batch
    pub inventories: Vec<InvEntry>, // remaining inventory to request (hashes + optional indexes)
    pub inv_was_full: bool,         // whether last INVMESSAGE had INVMESSAGE_LIMIT items
}

pub struct P2PServer {
    pub treechain: Arc<Mutex<TreeChain>>,
    pub pqp: Arc<Mutex<PQP>>,
    pub peers: Vec<String>,
    pub writers: Arc<Mutex<Vec<Arc<Mutex<OwnedWriteHalf>>>>>,
    pub best_peer: Arc<Mutex<Option<(String, u64)>>>, // (peer_addr, chain_length)
    sync_state: Arc<Mutex<Option<SyncState>>>,
}

impl P2PServer {
    pub fn new(treechain: Arc<Mutex<TreeChain>>, pqp: Arc<Mutex<PQP>>, peers: Vec<String>) -> Self {
        P2PServer {
            treechain,
            pqp,
            peers,
            writers: Arc::new(Mutex::new(Vec::new())),
            best_peer: Arc::new(Mutex::new(None)), // communicate only with the peer that has the longer tree
            sync_state: Arc::new(Mutex::new(None)),
        }
    }

    async fn build_block_locator(&self) -> Vec<String> {
        println!("Trying to lock treechain");
        let treechain = self.treechain.lock().await;
        println!("Locked treechain");
        let mut hashes: Vec<String> = treechain.blocks.keys().cloned().collect();
        hashes.reverse();
        hashes.truncate(10);
        drop(treechain);
        println!("Dropped treechain lock");
        hashes
    }

    async fn send_getblocks(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        println!("get blocks called");
        let block_locator = self.build_block_locator().await;
        let stopping_hash = "0".repeat(64);

        let message = serde_json::json!({
            "type": MESSAGE_TYPE_GETBLOCKS,
            "protocol_version": 70002,
            "block_locator": block_locator,
            "stopping_hash": stopping_hash
        });

        Self::send_message(writer, &message, "GETBLOCKS").await;
    }

    async fn send_getdatamessage(
        &self,
        writer: Arc<Mutex<OwnedWriteHalf>>,
        inventories: Vec<InvEntry>,
    ) {
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_GETDATA,
            "entries": inventories,
        });

        Self::send_message(writer, &message, "GETDATA").await;
    }

    pub async fn send_minedblock(&self, writer: Arc<Mutex<OwnedWriteHalf>>, block: Block) {
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_MINEDBLOCK ,
            "block": block ,
        });
        Self::send_message(writer, &message, "MINED_BLOCK").await;
    }

    async fn send_blockmessage(&self, writer: Arc<Mutex<OwnedWriteHalf>>, block: Block) {
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_BLOCK,
            "block": block,
        });

        Self::send_message(writer, &message, "BLOCK").await;
    }

    async fn send_message(writer: Arc<Mutex<OwnedWriteHalf>>, message: &Value, tag: &str) {
        let message_bytes = match serde_json::to_vec(message) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Failed to serialize {} message: {}", tag, e);
                return;
            }
        };

        let len = message_bytes.len() as u32;
        let mut locked_writer = writer.lock().await;

        if let Err(e) = locked_writer.write_all(&len.to_be_bytes()).await {
            eprintln!("Failed to send length for {}: {}", tag, e);
            return;
        }
        if let Err(e) = locked_writer.write_all(&message_bytes).await {
            eprintln!("Failed to send {} message: {}", tag, e);
            return;
        }
        if let Err(e) = locked_writer.flush().await {
            eprintln!("Failed to flush {} message: {}", tag, e);
            return;
        }

        println!("Sent {} message ({} bytes)", tag, len);
    }

    pub async fn listen(self: Arc<Self>, port: u16) {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).await.expect("Failed to bind");

        println!("Listening for peer to peer connections on {}", addr);
        self.clone().connect_to_peers().await;

        loop {
            let (stream, peer_addr) = listener.accept().await.unwrap();
            let self_clone = self.clone();
            tokio::spawn(async move {
                self_clone
                    .connect_socket(stream, peer_addr.to_string())
                    .await;
            });
        }
    }

    async fn connect_to_peers(self: Arc<Self>) {
        let peers = self.peers.clone();
        println!("peers in connect to peers: {:?}", peers);
        for peer in peers {
            println!("connect to peer: {:?}", peer);
            let result = timeout(Duration::from_secs(5), TcpStream::connect(&peer)).await;
            match result {
                Ok(Ok(stream)) => {
                    println!("Connected to peer {}", peer);
                    let self_clone = self.clone();
                    tokio::spawn(async move {
                        self_clone.connect_socket(stream, peer).await;
                    });
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to connect to peer {}: {}", peer, e);
                }
                Err(_) => {
                    eprintln!("Timeout while connecting to peer: {}", peer);
                }
            }
        }
    }

    async fn connect_socket(self: Arc<Self>, socket: TcpStream, peer_addr: String) {
        println!("New connection from {}", peer_addr);

        let (reader, writer) = socket.into_split();
        let writer = Arc::new(Mutex::new(writer));
        self.writers.lock().await.push(writer.clone());

        // Send initial CONNECTION_INFO
        {
            let treechain = self.treechain.lock().await;
            let chain_length = treechain.blocks.len();
            let last_block = treechain
                .blocks
                .get_index(chain_length - 1)
                .map(|(_, v)| v.clone());
            let message = serde_json::json!({
                "type": MESSAGE_TYPE_CONNECTION_INFO,
                "chain_length": chain_length,
                "last_block": last_block
            });

            let message_bytes = serde_json::to_vec(&message).unwrap();
            let len = message_bytes.len() as u32;

            let mut locked_writer = writer.lock().await;
            if let Err(e) = locked_writer.write_all(&len.to_be_bytes()).await {
                eprintln!("Failed to send message length to {}: {}", peer_addr, e);
                return;
            }
            if let Err(e) = locked_writer.write_all(&message_bytes).await {
                eprintln!("Failed to send treechain data to {}: {}", peer_addr, e);
                return;
            }
            if let Err(e) = locked_writer.flush().await {
                eprintln!("Failed to flush socket to {}: {}", peer_addr, e);
                return;
            }
            println!(
                "Sent chain_length: {}, last_block to {}",
                chain_length, peer_addr
            );
        }

        // Spawn message handler for the read half
        let self_clone = Arc::clone(&self);
        tokio::spawn(async move {
            self_clone
                .message_handler(reader, writer.clone(), peer_addr)
                .await;
        });
    }

    async fn is_best_peer(&self, addr: &str) -> bool {
        let best = self.best_peer.lock().await;
        match &*best {
            Some((best_addr, _)) if best_addr == addr => true,
            _ => false,
        }
    }

    async fn reset_sync_state(&self) {
        let mut st = self.sync_state.lock().await;
        *st = None;
    }

    async fn maybe_start_next_batch(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        println!("called start next batch");
        // take a look without holding the lock across await
        let next_batch: Option<Vec<InvEntry>>;
        let after_empty_inv_was_full: bool;

        {
            let mut st_opt = self.sync_state.lock().await;
            let st = match st_opt.as_mut() {
                Some(s) => s,
                None => return, // nothing to do
            };

            if st.pending != 0 {
                // current batch still in-flight; wait for BLOCKs
                return;
            }

            if st.inventories.is_empty() {
                // finished this INVMESSAGE's inventories
                after_empty_inv_was_full = st.inv_was_full;
                // clear state so next INVMESSAGE can reset it
                *st_opt = None;
            } else {
                // prepare next chunk
                let take = std::cmp::min(GETDATA_LIMIT as usize, st.inventories.len());
                let rest = st.inventories.split_off(take);
                let chunk = std::mem::replace(&mut st.inventories, rest);
                st.pending = chunk.len();
                next_batch = Some(chunk);
                drop(st_opt);
                // send GETDATA for this chunk
                self.send_getdatamessage(writer, next_batch.unwrap()).await;
                return;
            }
        }

        // Here, inventories were empty for this INVMESSAGE; decide whether to ask for more.
        if after_empty_inv_was_full {
            println!("Inventory exhausted but was full; asking for next batch via GETBLOCKS");
            self.send_getblocks(writer).await;
        } else {
            println!("✅ Finished syncing all available blocks from best peer");
        }
    }

    /// Called after each BLOCK arrival; decrements pending and, if the batch is complete,
    /// validates the tree and triggers next batch (or next GETBLOCKS).
    async fn on_block_delivered(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        println!("on_block_delivered called");
        let make_post_batch_call: bool;
        {
            let mut st_opt = self.sync_state.lock().await;
            let st = match st_opt.as_mut() {
                Some(s) => s,
                None => return, // not in a sync phase; ignore
            };
            if st.pending == 0 {
                return; // nothing expected currently
            }
            st.pending -= 1;
            make_post_batch_call = st.pending == 0;
        }

        // println!("make_post_batch_call: {}", make_post_batch_call);
        //
        if make_post_batch_call {
            // Validate PQP/tree after completing a batch
            {
                let treechain = self.treechain.lock().await;
                let pqp = self.pqp.lock().await;

                {
                    if !treechain.is_valid_pqp(&pqp) {
                        println!("❌ Invalid pqp detected after syncing a batch");
                        self.reset_sync_state().await;
                        return;
                    }
                    if !treechain.is_valid_tree(&pqp) {
                        println!("❌ Invalid tree detected after syncing a batch");
                        // reset state so we don't continue blindly
                        self.reset_sync_state().await;
                        return;
                    }
                }
            }
            // Start next batch or request next inventory window
            self.maybe_start_next_batch(writer).await;
        }
    }

    async fn message_handler(
        &self,
        mut reader: OwnedReadHalf,
        writer: Arc<Mutex<OwnedWriteHalf>>,
        peer_addr: String,
    ) {
        let mut buffer = vec![0u8; 4096];
        let mut received_data = Vec::new();

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    println!("Connection closed by peer: {}", peer_addr);
                    self.remove_writer(writer.clone()).await;

                    // If our best peer disconnected, clear selection and sync state
                    let mut best = self.best_peer.lock().await;
                    if best.as_ref().map(|(p, _)| p == &peer_addr).unwrap_or(false) {
                        println!(
                            "Best peer {} disconnected; clearing best_peer and sync_state",
                            peer_addr
                        );
                        *best = None;
                        drop(best);
                        self.reset_sync_state().await;
                    }
                    break;
                }
                Ok(n) => {
                    println!("Received {} bytes from {}", n, peer_addr);
                    received_data.extend_from_slice(&buffer[..n]);
                    while let Some((msg, remaining)) = Self::parse_message(&received_data) {
                        received_data = remaining;
                        match serde_json::from_value::<Value>(msg) {
                            Ok(data) => {
                                if let Some(msg_type) = data.get("type").and_then(|v| v.as_str()) {
                                    match msg_type {
                                        MESSAGE_TYPE_CONNECTION_INFO => {
                                            let chain_length_remote = data
                                                .get("chain_length")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(0);

                                            // remote last_block.timestamp (if provided)
                                            let remote_last_ts = data
                                                .get("last_block")
                                                .and_then(|lb| lb.get("timestamp"))
                                                .and_then(|t| t.as_u64());

                                            let (chain_length_local, local_last_ts) = {
                                                let treechain = self.treechain.lock().await;
                                                let len = treechain.blocks.len();

                                                // walk from the end until we find a non-placeholder (position != "")
                                                let mut last_ts: Option<u64> = None;
                                                for idx in (0..len).rev() {
                                                    if let Some((_h, b)) =
                                                        treechain.blocks.get_index(idx)
                                                    {
                                                        if !b.position.is_empty() {
                                                            last_ts = Some(b.timestamp as u64);
                                                            break;
                                                        }
                                                    }
                                                }
                                                (len, last_ts)
                                            };

                                            println!(
                                                "CONNECTION_INFO from {}: remote_len={}, local_len={}",
                                                peer_addr, chain_length_remote, chain_length_local
                                            );

                                            // 🕒 Compare timestamps if remote sent last_block
                                            const DAY_SECS: u64 = 24 * 60 * 60;
                                            // Choose best peer (longest chain). If selecting a new best, reset sync state.
                                            let mut best_peer = self.best_peer.lock().await;
                                            let mut changed = false;
                                            match &*best_peer {
                                                Some((current_peer, current_len)) => {
                                                    if chain_length_remote > *current_len {
                                                        println!(
                                                            "🌐 Selecting {} as best peer with chain length {} (prev {} @ {})",
                                                            peer_addr,
                                                            chain_length_remote,
                                                            current_len,
                                                            current_peer
                                                        );
                                                        *best_peer = Some((
                                                            peer_addr.clone(),
                                                            chain_length_remote,
                                                        ));
                                                        changed = true;
                                                    }
                                                }
                                                None => {
                                                    println!(
                                                        "🌐 Selecting {} as best peer with chain length {}",
                                                        peer_addr, chain_length_remote
                                                    );
                                                    *best_peer = Some((
                                                        peer_addr.clone(),
                                                        chain_length_remote,
                                                    ));
                                                    changed = true;
                                                }
                                            }
                                            drop(best_peer);

                                            if changed {
                                                // new best peer -> clear prior sync state
                                                self.reset_sync_state().await;
                                            }

                                            if self.is_best_peer(&peer_addr).await {
                                                if chain_length_remote > chain_length_local as u64 {
                                                    let remote_ahead =
                                                        match (remote_last_ts, local_last_ts) {
                                                            (Some(rts), Some(lts)) => {
                                                                rts.saturating_sub(lts) > DAY_SECS
                                                            }
                                                            (Some(_), None) => true, // local has no valid timestamp
                                                            _ => false,
                                                        };

                                                    if remote_ahead {
                                                        println!(
                                                            "⚠️ Remote ahead by >24h (remote_ts={:?}, local_ts={:?}); reinitializing to genesis",
                                                            remote_last_ts, local_last_ts
                                                        );

                                                        {
                                                            let mut treechain =
                                                                self.treechain.lock().await;
                                                            // keep genesis only
                                                            let genesis = treechain
                                                                .blocks
                                                                .get_index(0)
                                                                .map(|(_, b)| b.clone())
                                                                .expect("genesis block must exist");
                                                            let ghash = genesis.hash.clone();

                                                            treechain.blocks.clear();
                                                            treechain.children_map.clear();

                                                            // reinsert genesis
                                                            treechain.blocks.insert(
                                                                ghash.clone(),
                                                                genesis.clone(),
                                                            );
                                                            treechain
                                                                .children_map
                                                                .insert(ghash.clone(), vec![]);

                                                            // reset PQP and insert genesis entry
                                                            let mut pqp = self.pqp.lock().await;
                                                            *pqp = PQP::new();
                                                        }
                                                        println!(
                                                            "🌐 Requesting full sync from genesis via GETBLOCKS"
                                                        );
                                                        self.send_getblocks(writer.clone()).await;
                                                    } else {
                                                        println!(
                                                            "Best peer has longer chain, sending GETBLOCKS (timestamps close)"
                                                        );
                                                        self.send_getblocks(writer.clone()).await;
                                                    }
                                                } else {
                                                    println!(
                                                        "Our chain is equal or longer, no GETBLOCKS needed"
                                                    );
                                                }
                                            } else {
                                                println!(
                                                    "{} is not the best peer; ignoring sync start",
                                                    peer_addr
                                                );
                                            }
                                        }
                                        MESSAGE_TYPE_GETBLOCKS => {
                                            // Serve GETBLOCKS for *our* peers (i.e., when we are the sender)
                                            let block_locator = data
                                                .get("block_locator")
                                                .and_then(|v| v.as_array())
                                                .map(|arr| {
                                                    arr.iter()
                                                        .filter_map(|v| {
                                                            v.as_str().map(String::from)
                                                        })
                                                        .collect::<Vec<String>>()
                                                })
                                                .unwrap_or_default();
                                            println!(
                                                "Received GETBLOCKS from {} with block locator: {:?}",
                                                peer_addr, block_locator
                                            );
                                            self.send_invmessages(writer.clone(), block_locator)
                                                .await;
                                        }
                                        MESSAGE_TYPE_INVMESSAGE => {
                                            println!("received inv message");
                                            // Only handle INVMESSAGE from the best peer
                                            if !self.is_best_peer(&peer_addr).await {
                                                println!(
                                                    "Ignoring INVMESSAGE from {}, not best peer",
                                                    peer_addr
                                                );
                                                continue;
                                            }

                                            let inventories: Vec<InvEntry> =
                                                serde_json::from_value(
                                                    data.get("inventories")
                                                        .cloned()
                                                        .unwrap_or_default(),
                                                )
                                                .unwrap_or_default();

                                            println!(
                                                "Received INVMESSAGE from {} with {} entries",
                                                peer_addr,
                                                inventories.len()
                                            );

                                            // If already syncing (pending > 0 or have leftover inventories), ignore new inv
                                            // until current sync finishes, to avoid mixing windows.
                                            let should_ignore = {
                                                let st_opt = self.sync_state.lock().await;
                                                match &*st_opt {
                                                    Some(st)
                                                        if st.pending > 0
                                                            || !st.inventories.is_empty() =>
                                                    {
                                                        true
                                                    }
                                                    _ => false,
                                                }
                                            };
                                            if should_ignore {
                                                println!(
                                                    "Currently syncing another window; ignoring new INVMESSAGE"
                                                );
                                                continue;
                                            }

                                            // Initialize sync state for this window and kick off first GETDATA batch
                                            {
                                                let mut st_opt = self.sync_state.lock().await;
                                                *st_opt = Some(SyncState {
                                                    pending: 0,
                                                    inventories: inventories.clone(),
                                                    inv_was_full: inventories.len()
                                                        >= INVMESSAGE_LIMIT as usize,
                                                });
                                            }

                                            println!("calling start next batch ");
                                            // Begin first GETDATA batch. Following batches advance via BLOCK arrivals.
                                            self.maybe_start_next_batch(writer.clone()).await;
                                        }
                                        MESSAGE_TYPE_GETDATA => {
                                            // Serve blocks that our peer asked for
                                            let entries: Vec<InvEntry> = serde_json::from_value(
                                                data.get("entries").cloned().unwrap_or_default(),
                                            )
                                            .unwrap_or_default();
                                            println!(
                                                "Received GETDATA from {}, sending {} blocks",
                                                peer_addr,
                                                entries.len()
                                            );
                                            for entry in entries {
                                                let treechain = self.treechain.lock().await;
                                                if let Some(block) =
                                                    treechain.blocks.get(&entry.block_hash)
                                                {
                                                    self.send_blockmessage(
                                                        writer.clone(),
                                                        block.clone(),
                                                    )
                                                    .await;
                                                }
                                            }
                                        }
                                        MESSAGE_TYPE_BLOCK => {
                                            // Only accept blocks from best peer (prevents double-processing)
                                            if !self.is_best_peer(&peer_addr).await {
                                                println!(
                                                    "Ignoring BLOCK from {}, not best peer",
                                                    peer_addr
                                                );
                                                continue;
                                            }

                                            if let Some(block_val) = data.get("block") {
                                                let block: Block =
                                                    match serde_json::from_value(block_val.clone())
                                                    {
                                                        Ok(b) => b,
                                                        Err(_) => {
                                                            println!(
                                                                "❌ Failed to deserialize BLOCK"
                                                            );
                                                            continue;
                                                        }
                                                    };
                                                {
                                                    let mut treechain = self.treechain.lock().await;
                                                    let mut pqp = self.pqp.lock().await;
                                                    let pqp_entry =
                                                        TreeChain::parent_queue_entry_from_block(
                                                            &block,
                                                        );
                                                    pqp.add_entry(pqp_entry.clone());
                                                    let exist: bool = pqp
                                                        .pool
                                                        .iter()
                                                        .rev()
                                                        .take(CHILDREN as usize)
                                                        .any(|e| {
                                                            e.block_hash == pqp_entry.block_hash
                                                        });
                                                    if exist {
                                                        if treechain
                                                            .verify_and_add_block(&block.clone())
                                                        {
                                                            println!(
                                                                "✅ Block {} added successfully",
                                                                block.hash
                                                            );
                                                        } else {
                                                            println!(
                                                                "❌ Failed to add block {} (duplicate or invalid)",
                                                                block.hash
                                                            );
                                                            if pqp
                                                                .remove_pqp_entry(pqp_entry.clone())
                                                            {
                                                                println!(
                                                                    "✅ removed the pqp entry "
                                                                );
                                                            } else {
                                                                println!(
                                                                    "❌ failed to remove the pqp entry "
                                                                );
                                                            }
                                                            // Even if duplicate, we still count delivery against pending.
                                                        }
                                                    } else {
                                                        println!(
                                                            "ParentQueueEntry of the current Block is not added {}",
                                                            pqp_entry.block_hash
                                                        );
                                                    }

                                                    drop(treechain);
                                                    drop(pqp);
                                                }
                                                // Update batch accounting and trigger follow-up when batch completes
                                                self.on_block_delivered(writer.clone()).await;
                                            }
                                        }
                                        MESSAGE_TYPE_MINEDBLOCK => {
                                            if let Some(block_val) = data.get("block") {
                                                let block: Block =
                                                    match serde_json::from_value(block_val.clone())
                                                    {
                                                        Ok(b) => b,
                                                        Err(_) => {
                                                            println!(
                                                                "❌ Failed to deserialize BLOCK"
                                                            );
                                                            continue;
                                                        }
                                                    };
                                                {
                                                    let mut treechain = self.treechain.lock().await;
                                                    let mut pqp = self.pqp.lock().await;
                                                    let pqp_entry =
                                                        TreeChain::parent_queue_entry_from_block(
                                                            &block,
                                                        );
                                                    pqp.add_entry(pqp_entry.clone());
                                                    let exist: bool = pqp
                                                        .pool
                                                        .iter()
                                                        .rev()
                                                        .take(CHILDREN as usize)
                                                        .any(|e| {
                                                            e.block_hash == pqp_entry.block_hash
                                                        });
                                                    if exist {
                                                        if treechain
                                                            .verify_and_add_block(&block.clone())
                                                        {
                                                            println!(
                                                                "✅ Block {} added successfully",
                                                                block.hash
                                                            );
                                                        } else {
                                                            println!(
                                                                "❌ Failed to add block {} (duplicate or invalid)",
                                                                block.hash
                                                            );
                                                            if pqp
                                                                .remove_pqp_entry(pqp_entry.clone())
                                                            {
                                                                println!(
                                                                    "✅ removed the pqp entry "
                                                                );
                                                            } else {
                                                                println!(
                                                                    "❌ failed to remove the pqp entry "
                                                                );
                                                            }
                                                        }
                                                    } else {
                                                        println!(
                                                            "ParentQueueEntry of the current Block is not added"
                                                        );

                                                        if let Some(latest) = pqp.latest() {
                                                            if block.pqp_entry.queue_index
                                                                > ((2 * CHILDREN as usize) //donot change CHILDREN into MAX_CHILDREN
                                                                    + (latest.queue_index as usize))
                                                                    .try_into()
                                                                    .unwrap()
                                                            {
                                                                println!(
                                                                    "sending the get_blocks message:"
                                                                );
                                                                self.send_getblocks(writer.clone())
                                                                    .await;
                                                            }
                                                        }
                                                    }

                                                    drop(treechain);
                                                    drop(pqp);
                                                }
                                            }
                                        }
                                        _ => {
                                            println!("Other type from {}: {:?}", peer_addr, data);
                                        }
                                    }
                                }
                            }
                            Err(e) => eprintln!("Invalid JSON from {}: {}", peer_addr, e),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Read error from {}: {}", peer_addr, e);
                    self.remove_writer(writer.clone()).await;

                    // If our best peer errored, clear selection and sync state
                    let mut best = self.best_peer.lock().await;
                    if best.as_ref().map(|(p, _)| p == &peer_addr).unwrap_or(false) {
                        println!(
                            "Best peer {} errored; clearing best_peer and sync_state",
                            peer_addr
                        );
                        *best = None;
                        drop(best);
                        self.reset_sync_state().await;
                    }
                    break;
                }
            }
        }
    }

    async fn send_invmessages(
        &self,
        writer: Arc<Mutex<OwnedWriteHalf>>,
        block_locator: Vec<String>,
    ) {
        let treechain = self.treechain.lock().await;

        // find first matching locator
        let mut start_index = None;
        for locator in block_locator {
            if let Some((idx, (hash, _))) = treechain
                .blocks
                .iter()
                .enumerate()
                .find(|(_, (hash, _))| *hash == &locator)
            {
                start_index = Some(idx);
                break;
            }
        }

        if let Some(start) = start_index {
            let mut inventories = Vec::new();
            for (idx, (hash, _)) in treechain.blocks.iter().enumerate().skip(start + 1) {
                inventories.push(InvEntry {
                    queue_index: idx as u32,
                    block_hash: hash.clone(),
                });
                if inventories.len() >= (INVMESSAGE_LIMIT as usize) {
                    break;
                }
            }

            let message = serde_json::json!({
                "type": MESSAGE_TYPE_INVMESSAGE,
                "inventories": inventories
            });

            drop(treechain);
            Self::send_message(writer, &message, "INVMESSAGE").await;
        }
    }

    async fn remove_writer(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let mut writers = self.writers.lock().await;
        writers.retain(|w| !Arc::ptr_eq(w, &writer));
        println!("Removed disconnected writer");
    }

    fn parse_message(data: &[u8]) -> Option<(Value, Vec<u8>)> {
        if data.len() < 4 {
            return None;
        }
        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + len {
            return None;
        }
        let msg_data = &data[4..4 + len];
        match serde_json::from_slice::<Value>(msg_data) {
            Ok(msg) => Some((msg, data[4 + len..].to_vec())),
            Err(_) => None,
        }
    }
}

pub fn start_p2p_server(
    port: String,
    peers: String,
    treechain: Arc<Mutex<TreeChain>>,
    pqp: Arc<Mutex<PQP>>,
) -> Arc<P2PServer> {
    let peer_list: Vec<String> = peers
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let server = Arc::new(P2PServer::new(treechain, pqp, peer_list));
    let server_clone = Arc::clone(&server);
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        println!("peers: {}", peers);
        runtime.block_on(async move {
            server_clone
                .listen(port.parse::<u16>().expect("Invalid port number"))
                .await;
        })
    });

    server
}
