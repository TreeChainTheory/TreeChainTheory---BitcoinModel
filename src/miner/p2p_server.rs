//1st terminal cargo run --bin TreeChainTheorey
//2nd terminal ALIGN=2 HTTP_PORT=3002 P2P_PORT=5002 cargo run --bin TreeChainTheorey
//3rd terminal ALIGN=3 HTTP_PORT=3003 P2P_PORT=5003 cargo run --bin TreeChainTheorey
//4th terminal ALIGN=1 HTTP_PORT=3004 P2P_PORT=5004 cargo run --bin TreeChainTheorey
//5th terminal ALIGN=2 HTTP_PORT=3005 P2P_PORT=5005 cargo run --bin TreeChainTheorey
//6th terminal ALIGN=3 HTTP_PORT=3006 P2P_PORT=5006 cargo run --bin TreeChainTheorey

use crate::config::{CHILDREN, GETDATA_LIMIT, INVMESSAGE_LIMIT};
use crate::treechain::block::Block;
use crate::treechain::treechain::ParentQueueEntry;
use crate::treechain::treechain::{PQP, TreeChain};
use crate::wallet::transaction::Transaction;
use crate::wallet::transaction_pool::TransactionPool;
use crate::wallet::utxo::UtxoSet;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::Mutex as SyncMutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use std::time::Instant;
use tokio::time::{Duration, timeout};

const DAY_SECS: u64 = 24 * 60 * 60;
const DAY_MS: u64 = DAY_SECS * 1_000;
const MESSAGE_TYPE_CONNECTION_INFO: &str = "CONNECTION_INFO";
// Longest-tree rule: nodes announce the size and digest of their trees, and a node that keeps
// seeing a better tree fetches it, replays it through all validation rules, and switches to it.
const MESSAGE_TYPE_TREE_STATUS: &str = "TREE_STATUS";
const MESSAGE_TYPE_GET_TREE: &str = "GET_TREE";
const MESSAGE_TYPE_TREE: &str = "TREE";
const TREE_STATUS_INTERVAL_SECS: u64 = 5; // how often a node announces its tree
const TREE_SWITCH_GRACE_SECS: u64 = 8; // how long a better tree must persist before switching
const MESSAGE_TYPE_GETBLOCKS: &str = "GETBLOCKS";
const MESSAGE_TYPE_INVMESSAGE: &str = "INVMESSAGE";
const MESSAGE_TYPE_GETDATA: &str = "GETDATA";
const MESSAGE_TYPE_BLOCK: &str = "BLOCK";
const MESSAGE_TYPE_MINEDBLOCK: &str = "MINED_BLOCK";
const MESSAGE_TYPE_GET_PQP: &str = "GET_PQP";
const MESSAGE_TYPE_PQP_RESPONSE: &str = "PQP_RESPONSE";
const MESSAGE_TYPE_REGISTER: &str = "REGISTER";
const MESSAGE_TYPE_UPDATE: &str = "UPDATE";
const MESSAGE_TYPE_PEER_LIST: &str = "PEER_LIST";
const MESSAGE_TYPE_GET_PEER_LIST: &str = "GET_PEER_LIST";
const MESSAGE_TYPE_TRANSACTION: &str = "TRANSACTION";
const MESSAGE_TYPE_GET_TRANSACTION_POOL: &str = "GET_TRANSACTION_POOL";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvEntry {
    pub queue_index: u32,
    pub block_hash: String,
}

pub struct SyncState {
    pub pending: usize,
    pub inventories: Vec<InvEntry>,
    pub inv_was_full: bool,
}

/// Latest tree status a peer announced, with the connection it came on.
#[derive(Clone)]
struct PeerTreeStatus {
    count: u64,
    digest: String,
    received_at: Instant,
    writer: Arc<Mutex<OwnedWriteHalf>>,
}

pub struct P2PServer {
    pub treechain: Arc<Mutex<TreeChain>>,
    pub pqp: Arc<Mutex<PQP>>,
    pub utxo_set: Arc<Mutex<UtxoSet>>,
    pub txn_pool: Arc<Mutex<TransactionPool>>,
    pub peer_lengths: Arc<Mutex<HashMap<String, (u64, u64)>>>,
    pub connected_peers: Arc<Mutex<HashSet<String>>>,
    pub peer_writers: Arc<Mutex<HashMap<String, Arc<Mutex<OwnedWriteHalf>>>>>,
    pub registry_writer: Arc<Mutex<Option<Arc<Mutex<OwnedWriteHalf>>>>>,
    pub own_addr: String,
    sync_state: Arc<Mutex<Option<SyncState>>>,
    pub current_mining_position: Arc<SyncMutex<Option<String>>>,
    pub abort_mining: Arc<SyncMutex<bool>>,
    pub ibd_or_online_state: Arc<Mutex<bool>>,
    pub chain_length: Arc<SyncMutex<u64>>,
    pending_blocks: Arc<Mutex<HashMap<String, Block>>>, // New: Store blocks with missing parents
    recently_processed_blocks: Arc<Mutex<VecDeque<String>>>,
    peer_tree_status: Arc<Mutex<HashMap<String, PeerTreeStatus>>>,
    better_tree_since: Arc<Mutex<Option<Instant>>>,
    tree_switch_in_progress: Arc<Mutex<bool>>,
    started_at: Instant,
    mining_hold_until: Arc<SyncMutex<Option<Instant>>>,
    // Test hook (TEST_ISOLATE="start:duration", seconds after start): drop incoming blocks and tree
    // messages in that window while still mining, as a node cut off from the network would. Used
    // only to test that the longest-tree rule repairs a fork; unset in normal runs.
    test_isolation: Option<(u64, u64)>,
}

impl P2PServer {
    pub fn new(
        treechain: Arc<Mutex<TreeChain>>,
        pqp: Arc<Mutex<PQP>>,
        utxo_set: Arc<Mutex<UtxoSet>>,
        txn_pool: Arc<Mutex<TransactionPool>>,
        _peers: Vec<String>,
        current_mining_position: Arc<SyncMutex<Option<String>>>,
        abort_mining: Arc<SyncMutex<bool>>,
        own_addr: String,
        chain_length: Arc<SyncMutex<u64>>,
    ) -> Self {
        P2PServer {
            treechain,
            pqp,
            utxo_set,
            txn_pool,
            peer_lengths: Arc::new(Mutex::new(HashMap::new())),
            connected_peers: Arc::new(Mutex::new(HashSet::new())),
            peer_writers: Arc::new(Mutex::new(HashMap::new())),
            registry_writer: Arc::new(Mutex::new(None)),
            own_addr,
            sync_state: Arc::new(Mutex::new(None)),
            current_mining_position,
            abort_mining,
            ibd_or_online_state: Arc::new(Mutex::new(false)),
            chain_length,
            pending_blocks: Arc::new(Mutex::new(HashMap::new())),
            // NEW: Initialize recently processed blocks set
            recently_processed_blocks: Arc::new(Mutex::new(VecDeque::new())),
            peer_tree_status: Arc::new(Mutex::new(HashMap::new())),
            better_tree_since: Arc::new(Mutex::new(None)),
            tree_switch_in_progress: Arc::new(Mutex::new(false)),
            started_at: Instant::now(),
            mining_hold_until: Arc::new(SyncMutex::new(None)),
            test_isolation: std::env::var("TEST_ISOLATE").ok().and_then(|v| {
                let mut parts = v.split(':').map(|x| x.trim().parse::<u64>());
                match (parts.next(), parts.next()) {
                    (Some(Ok(start)), Some(Ok(duration))) => Some((start, duration)),
                    _ => None,
                }
            }),
        }
    }

    /// True for a short while after the node switched trees: the miner waits until the node has
    /// caught up with the blocks mined during the switch, instead of competing with them.
    pub fn mining_on_hold(&self) -> bool {
        match *self.mining_hold_until.lock().unwrap() {
            Some(until) => Instant::now() < until,
            None => false,
        }
    }

    fn isolated_now(&self) -> bool {
        match self.test_isolation {
            Some((start, duration)) => {
                let t = self.started_at.elapsed().as_secs();
                t >= start && t < start + duration
            }
            None => false,
        }
    }

    async fn local_tree_status(&self) -> (u64, String) {
        let tree = self.treechain.lock().await;
        let status = (tree.count as u64, tree.tree_digest());
        drop(tree);
        status
    }

    async fn broadcast_tree_status(&self) {
        let (count, digest) = self.local_tree_status().await;
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_TREE_STATUS,
            "own_addr": self.own_addr,
            "count": count,
            "digest": digest,
        });
        // copy the writers so the peer map is not held while sending
        let writers: Vec<Arc<Mutex<OwnedWriteHalf>>> =
            self.peer_writers.lock().await.values().cloned().collect();
        for writer in writers {
            Self::send_message(writer, &message, "TREE_STATUS").await;
        }
    }

    /// Fork choice. If some peer has announced a better tree (more blocks, or as many blocks and a
    /// smaller digest) continuously for TREE_SWITCH_GRACE_SECS, request that peer's whole tree.
    /// The grace period lets blocks that are still in flight arrive the normal way.
    async fn check_fork_choice(&self) {
        if *self.tree_switch_in_progress.lock().await {
            return;
        }
        let (local_count, local_digest) = self.local_tree_status().await;
        let now = Instant::now();
        let best = {
            let statuses = self.peer_tree_status.lock().await;
            statuses
                .iter()
                .filter(|(_, st)| {
                    now.duration_since(st.received_at)
                        < Duration::from_secs(3 * TREE_STATUS_INTERVAL_SECS)
                })
                .filter(|(_, st)| {
                    TreeChain::is_better_tree(st.count, &st.digest, local_count, &local_digest)
                })
                .max_by(|a, b| {
                    a.1.count
                        .cmp(&b.1.count)
                        .then_with(|| b.1.digest.cmp(&a.1.digest))
                })
                .map(|(addr, st)| (addr.clone(), st.clone()))
        };
        let mut since = self.better_tree_since.lock().await;
        match best {
            None => *since = None,
            Some((addr, st)) => {
                let started = *since.get_or_insert(now);
                if now.duration_since(started) >= Duration::from_secs(TREE_SWITCH_GRACE_SECS) {
                    *since = None;
                    drop(since);
                    println!(
                        "🌳 Peer {} has a better tree ({} blocks, digest {}) than ours ({} blocks, digest {}); requesting it",
                        addr, st.count, &st.digest[..12], local_count, &local_digest[..12]
                    );
                    let message = serde_json::json!({ "type": MESSAGE_TYPE_GET_TREE });
                    Self::send_message(st.writer.clone(), &message, "GET_TREE").await;
                }
            }
        }
    }

    /// Replay a peer's tree from genesis through all validation rules and, if it is valid and
    /// still better than ours, switch to it: tree, PQP and UTXO set are replaced at once, our
    /// transactions that the new tree does not contain go back to the mempool if still valid, and
    /// the miner restarts on the new tree.
    async fn switch_to_tree(self: Arc<Self>, blocks: Vec<Block>, from: Arc<Mutex<OwnedWriteHalf>>) {
        {
            let mut busy = self.tree_switch_in_progress.lock().await;
            if *busy {
                return;
            }
            *busy = true;
        }
        match TreeChain::rebuild_from_blocks(&blocks) {
            Err(e) => println!("❌ Received tree rejected: {}", e),
            Ok((new_tree, new_pqp, new_utxo)) => {
                let new_count = new_tree.count as u64;
                let new_digest = new_tree.tree_digest();
                // lock order: tree -> pqp -> utxo_set -> txn_pool
                let mut tree = self.treechain.lock().await;
                let old_count = tree.count as u64;
                let old_digest = tree.tree_digest();
                if !TreeChain::is_better_tree(new_count, &new_digest, old_count, &old_digest) {
                    println!(
                        "Received tree ({} blocks) is not better than ours ({} blocks); keeping ours",
                        new_count, old_count
                    );
                } else {
                    let mut pqp = self.pqp.lock().await;
                    let mut utxo_set = self.utxo_set.lock().await;
                    let mut txn_pool = self.txn_pool.lock().await;
                    let new_txids: HashSet<String> = new_tree
                        .blocks
                        .values()
                        .flat_map(|b| b.tx.iter().map(|t| t.txid.clone()))
                        .collect();
                    let new_txids_blocks: HashSet<String> =
                        new_tree.blocks.keys().cloned().collect();
                    // transactions of our blocks that the new tree drops, then our mempool
                    let mut readd: Vec<Transaction> = tree
                        .blocks
                        .values()
                        .filter(|b| !b.position.is_empty())
                        .flat_map(|b| b.tx.iter().skip(1).cloned())
                        .filter(|t| !new_txids.contains(&t.txid))
                        .collect();
                    let mut pooled: Vec<_> = txn_pool.pool.values().collect();
                    pooled.sort_by_key(|e| e.added_time);
                    readd.extend(
                        pooled
                            .into_iter()
                            .map(|e| e.tx.clone())
                            .filter(|t| !new_txids.contains(&t.txid)),
                    );
                    *tree = new_tree;
                    *pqp = new_pqp;
                    *utxo_set = new_utxo;
                    let mut new_pool = TransactionPool::new();
                    let mut kept = 0;
                    for tx in readd {
                        if new_pool.add_transaction(tx, &utxo_set, &tree).is_ok() {
                            kept += 1;
                        }
                    }
                    *txn_pool = new_pool;
                    {
                        let mut chain_length = self.chain_length.lock().unwrap();
                        *chain_length = tree.count as u64;
                    }
                    drop(txn_pool);
                    drop(utxo_set);
                    drop(pqp);
                    drop(tree);
                    *self.abort_mining.lock().unwrap() = true; // restart the miner on the new tree
                    // keep queued blocks that the new tree does not contain yet: many are the
                    // newest blocks of the branch we just joined, and the monitor retries them
                    // every second, so we catch up without another full download
                    self.pending_blocks
                        .lock()
                        .await
                        .retain(|hash, _| !new_txids_blocks.contains(hash));
                    *self.better_tree_since.lock().await = None;
                    *self.mining_hold_until.lock().unwrap() =
                        Some(Instant::now() + Duration::from_secs(3));
                    // a download that was running for the old tree no longer applies
                    *self.sync_state.lock().await = None;
                    *self.ibd_or_online_state.lock().await = false;
                    self.update_registry_chain_length().await;
                    println!(
                        "🔀 Switched to a better tree: {} blocks (ours had {}), {} transactions back in the mempool",
                        new_count, old_count, kept
                    );
                    // fetch the blocks the peer mined while we were replaying its tree
                    self.send_getblocks(from).await;
                }
            }
        }
        *self.tree_switch_in_progress.lock().await = false;
    }

    pub async fn get_chain_info(&self) -> (u64, Option<u64>) {
        let treechain = self.treechain.lock().await;
        let len = treechain.count as u64;
        let mut last_ts: Option<u64> = None;
        for (_, b) in treechain.blocks.iter().rev() {
            if !b.position.is_empty() {
                last_ts = Some(b.timestamp as u64);
                break;
            }
        }
        drop(treechain);
        (len, last_ts)
    }

    pub async fn get_chain_info_precise(&self) -> (u64, Option<u64>) {
        let treechain = self.treechain.lock().await;
        let mut len: u64 = 0;
        let mut last_ts: Option<u64> = None;
        for (_, b) in treechain.blocks.iter() {
            if !b.position.is_empty() {
                len += 1;
                last_ts = Some(b.timestamp as u64);
            }
        }
        drop(treechain);
        (len, last_ts)
    }

    pub async fn update_reg_cl_precise(&self) {
        let opt_writer = self.registry_writer.lock().await.clone();
        if let Some(writer) = opt_writer {
            let (len, ts) = self.get_chain_info_precise().await;
            let message = serde_json::json!({
                "type": MESSAGE_TYPE_UPDATE,
                "addr": self.own_addr,
                "chain_length": len,
                "last_timestamp": ts.unwrap_or(0),
            });
            Self::send_message(writer, &message, "UPDATE").await;
        }
    }

    pub async fn update_registry_chain_length(&self) {
        let opt_writer = self.registry_writer.lock().await.clone();
        if let Some(writer) = opt_writer {
            let (len, ts) = self.get_chain_info().await;
            let message = serde_json::json!({
                "type": MESSAGE_TYPE_UPDATE,
                "addr": self.own_addr,
                "chain_length": len,
                "last_timestamp": ts.unwrap_or(0),
            });
            Self::send_message(writer, &message, "UPDATE").await;
        }
    }

    async fn connect_to_registry(self: Arc<Self>, own_addr: String) {
        match timeout(Duration::from_secs(5), TcpStream::connect("0.0.0.0:8080")).await {
            Ok(Ok(stream)) => {
                println!("Connected to ports server");
                let (reader, writer) = stream.into_split();
                let writer = Arc::new(Mutex::new(writer));
                *self.registry_writer.lock().await = Some(writer.clone());
                let (chain_length, last_ts) = self.get_chain_info().await;
                let register_message = serde_json::json!({
                    "type": MESSAGE_TYPE_REGISTER,
                    "addr": own_addr,
                    "chain_length": chain_length,
                    "last_timestamp": last_ts.unwrap_or(0),
                });
                Self::send_message(writer.clone(), &register_message, "REGISTER").await;
                let self_clone = self.clone();
                tokio::spawn(async move {
                    self_clone.handle_registry_messages(reader).await;
                });
                let self_clone2 = self.clone();
                tokio::spawn(async move {
                    self_clone2.monitor_sync().await;
                });
            }
            _ => {
                eprintln!("Failed to connect to ports server");
            }
        }
    }

    pub async fn handle_registry_messages(self: Arc<Self>, mut reader: OwnedReadHalf) {
        let mut buffer = vec![0u8; 4096];
        let mut received_data = Vec::new();
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    println!("Ports server connection closed");
                    break;
                }
                Ok(n) => {
                    received_data.extend_from_slice(&buffer[..n]);
                    while let Some((msg, remaining)) = Self::parse_message(&received_data) {
                        received_data = remaining;
                        let data: Value = serde_json::from_value(msg).unwrap_or_default();
                        println!("Received message from ports server: {:?}", data);
                        if data.get("type").and_then(|v| v.as_str()) == Some(MESSAGE_TYPE_PEER_LIST)
                        {
                            let empty: Vec<Value> = Vec::new();
                            let peers_arr = data
                                .get("peers")
                                .and_then(|v| v.as_array())
                                .unwrap_or(&empty);
                            println!("Received PEER_LIST with {} peers", peers_arr.len());
                            let mut map = HashMap::new();
                            for p in peers_arr {
                                let addr = p
                                    .get("addr")
                                    .and_then(|v| v.as_str())
                                    .map(String::from)
                                    .unwrap_or_default();
                                let chain_length =
                                    p.get("chain_length").and_then(|v| v.as_u64()).unwrap_or(0);
                                let last_timestamp = p
                                    .get("last_timestamp")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                if !addr.is_empty() {
                                    println!(
                                        "Processing peer {}: chain_length={}, last_timestamp={}",
                                        addr, chain_length, last_timestamp
                                    );
                                    map.insert(addr, (chain_length, last_timestamp));
                                }
                            }
                            let treechain = self.treechain.lock().await;

                            let mut last_ts: Option<u64> = None;
                            for (_, b) in treechain.blocks.iter().rev() {
                                if !b.position.is_empty() {
                                    last_ts = Some(b.timestamp as u64);
                                    break;
                                }
                            }
                            drop(treechain);
                            let peer_lengths = self.peer_lengths.lock().await;
                            let connected_peers = self.connected_peers.lock().await;
                            println!("Currently connected peers {:?}", connected_peers);
                            let peers_to_connect: Vec<String> = map
                                .keys()
                                .cloned()
                                .filter(|a| {
                                    let formatted = format!("{}", a);
                                    let peer_info = peer_lengths.get(&formatted);
                                    let self_len = *self.chain_length.lock().unwrap();
                                    let self_ts = last_ts;
                                    let should_connect = a != &self.own_addr
                                        && !connected_peers.contains(&formatted)
                                        && peer_info.map_or(true, |&(peer_len, peer_ts)| {
                                            peer_len > self_len
                                                || (peer_len == self_len
                                                    && peer_ts > self_ts.unwrap_or(0))
                                        });
                                    println!("Peer {}: should_connect={}", a, should_connect);
                                    should_connect
                                })
                                .collect();
                            drop(connected_peers);
                            println!(
                                "Connecting to {} peers: {:?}",
                                peers_to_connect.len(),
                                peers_to_connect
                            );
                            drop(peer_lengths);
                            *self.peer_lengths.lock().await = map;
                            let self_clone = self.clone();
                            let peers_to_connect_clone = peers_to_connect.clone();
                            tokio::spawn(async move {
                                let jitter = rand::thread_rng().gen_range(100..500);
                                tokio::time::sleep(Duration::from_millis(jitter)).await;
                                self_clone.connect_to_peers(peers_to_connect_clone).await;
                            });
                        } else {
                            println!(
                                "Unexpected message type from ports server: {:?}",
                                data.get("type")
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Read error from ports server: {}", e);
                    break;
                }
            }
        }
    }

    async fn monitor_sync(self: Arc<Self>) {
        let mut tick: u64 = 0;
        loop {
            if tick % TREE_STATUS_INTERVAL_SECS == 0 {
                self.broadcast_tree_status().await;
            }
            tick += 1;
            self.check_fork_choice().await;
            if let Some((best_addr, best_len, _best_ts)) = self.get_current_best_peer().await {
                let (local_len, _local_ts) = self.get_chain_info().await;
                if best_len > local_len {
                    let mut ibd = self.ibd_or_online_state.lock().await;
                    if !*ibd {
                        *ibd = true;
                        drop(ibd);
                        // Clone the writer and release the peer map before send_getblocks,
                        // which locks the tree
                        let writer = self.peer_writers.lock().await.get(&best_addr).cloned();
                        if let Some(w) = writer {
                            println!(
                                "Triggering sync with best peer {} (len={})",
                                best_addr, best_len
                            );
                            self.send_getblocks(w).await;
                        }
                    }
                } else {
                    let mut ibd = self.ibd_or_online_state.lock().await;
                    if *ibd {
                        *ibd = false;
                        println!("Chain lengths equalized; exiting IBD mode");
                    }
                }
            }
            // Check pending blocks and retry adding them. The pending map is only held while
            // taking the blocks out and putting failures back, never together with the state locks.
            let blocks: Vec<Block> = {
                let mut pending_blocks = self.pending_blocks.lock().await;
                let taken = pending_blocks.values().cloned().collect();
                pending_blocks.clear();
                taken
            };
            if !blocks.is_empty() {
                // retry in slot order, the order in which the window admits blocks
                let mut blocks = blocks;
                blocks.sort_by_key(|b| b.pqp_entry.queue_index);
                let mut requeue: Vec<Block> = Vec::new();
                let mut treechain = self.treechain.lock().await;
                let mut pqp = self.pqp.lock().await;
                let mut utxo_set = self.utxo_set.lock().await;
                let mut txn_pool = self.txn_pool.lock().await;
                for block in blocks {
                    // already in the tree, slot taken by another block, or parent already
                    // retired: such a block can never be added, so stop retrying it
                    let in_tree = treechain.get_block(&block.hash).is_some();
                    let slot_taken = treechain
                        .blocks
                        .get_index(block.pqp_entry.queue_index as usize)
                        .map(|(h, b)| !b.position.is_empty() && *h != block.hash)
                        .unwrap_or(false);
                    let parent_retired =
                        match (treechain.get_block(&block.parent_hash), pqp.current_parent()) {
                            (Some(parent), Some(current)) => {
                                parent.pqp_entry.queue_index < current.queue_index
                            }
                            _ => false,
                        };
                    if in_tree || slot_taken || parent_retired {
                        println!(
                            "Dropping pending block {} (in tree: {}, slot taken: {}, parent retired: {})",
                            block.hash, in_tree, slot_taken, parent_retired
                        );
                        continue;
                    }
                    let pqp_entry = TreeChain::parent_queue_entry_from_block(&block);
                    let pool_before = pqp.pool.clone();
                    pqp.add_entry_to_pqp(pqp_entry.clone(), &treechain);
                    // as on the other paths, a block joins the tree only if the parent queue took it
                    let accepted = pqp.pool.iter().any(|e| e.block_hash == pqp_entry.block_hash);
                    if !accepted {
                        println!(
                            "❌ Parent queue rejected pending block {}; re-queueing",
                            block.hash
                        );
                        requeue.push(block);
                    } else if treechain.verify_and_add_block_to_tree(
                        &block,
                        &mut utxo_set,
                        &mut txn_pool,
                    ) {
                        println!("✅ Added pending block {} from retry", block.hash);
                        {
                            let mut chain_length = self.chain_length.lock().unwrap();
                            *chain_length = treechain.count as u64;
                            drop(chain_length);
                        }
                        drop(treechain);
                        drop(pqp);
                        drop(utxo_set);
                        drop(txn_pool);
                        self.update_registry_chain_length().await;
                        treechain = self.treechain.lock().await;
                        pqp = self.pqp.lock().await;
                        utxo_set = self.utxo_set.lock().await;
                        txn_pool = self.txn_pool.lock().await;
                    } else {
                        // the block is invalid: undo whatever its entry did to the window
                        pqp.pool = pool_before;
                        println!("❌ Failed to add pending block {}; re-queueing", block.hash);
                        requeue.push(block);
                    }
                }
                drop(treechain);
                drop(pqp);
                drop(utxo_set);
                drop(txn_pool);
                if !requeue.is_empty() {
                    let mut pending_blocks = self.pending_blocks.lock().await;
                    for block in requeue {
                        pending_blocks.insert(block.hash.clone(), block);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await; // Reduced interval for faster sync
        }
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
            eprintln!("Failed to write length for {} message: {}", tag, e);
            return;
        }
        if let Err(e) = locked_writer.write_all(&message_bytes).await {
            eprintln!("Failed to write {} message: {}", tag, e);
            return;
        }
        if let Err(e) = locked_writer.flush().await {
            eprintln!("Failed to flush {} message: {}", tag, e);
            return;
        }
        println!("Sent {} message ({} bytes)", tag, len);
    }

    fn parse_message(data: &[u8]) -> Option<(Value, Vec<u8>)> {
        if data.len() < 4 {
            println!("Data too short for message length: {} bytes", data.len());
            return None;
        }
        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + len {
            println!(
                "Data incomplete: have {} bytes, need {} bytes",
                data.len(),
                4 + len
            );
            return None;
        }
        let msg_data = &data[4..4 + len];
        match serde_json::from_slice::<Value>(msg_data) {
            Ok(msg) => {
                println!("Parsed message of type {:?}", msg.get("type"));
                Some((msg, data[4 + len..].to_vec()))
            }
            Err(e) => {
                println!("Failed to parse message: {}", e);
                None
            }
        }
    }

    async fn send_getblocks(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        println!("Sending GETBLOCKS");
        let treechain = self.treechain.lock().await;
        let block_locator: Vec<String> = treechain
            .blocks
            .iter()
            .rev()
            .filter(|(_, b)| !b.position.is_empty())
            .take(10)
            .map(|(h, _)| h.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        drop(treechain);
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_GETBLOCKS,
            "block_locator": block_locator,
        });
        Self::send_message(writer, &message, "GETBLOCKS").await;
    }

    async fn send_getdatamessage(&self, writer: Arc<Mutex<OwnedWriteHalf>>, hashes: Vec<InvEntry>) {
        println!("Sending GETDATA for {} entries", hashes.len());
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_GETDATA,
            "hashes": hashes
        });
        Self::send_message(writer, &message, "GETDATA").await;
    }

    async fn send_get_pqp(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_GET_PQP
        });
        Self::send_message(writer, &message, "GET_PQP").await;
    }

    async fn send_get_txnpool(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_GET_TRANSACTION_POOL
        });
        Self::send_message(writer, &message, "GET_TXN_POOL").await;
    }

    async fn send_pqp_response(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let pqp = self.pqp.lock().await;
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_PQP_RESPONSE,
            "pool": pqp.pool.clone(),
        });
        drop(pqp);
        Self::send_message(writer, &message, "PQP_RESPONSE").await;
    }

    pub async fn send_minedblock(&self, block: Block) {
        // Copy the peer list so the peer map is not held during network writes
        let writers: Vec<(String, Arc<Mutex<OwnedWriteHalf>>)> = self
            .peer_writers
            .lock()
            .await
            .iter()
            .map(|(a, w)| (a.clone(), w.clone()))
            .collect();
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_MINEDBLOCK,
            "block": block,
        });
        for (addr, w) in writers {
            println!("Broadcasting MINED_BLOCK to {}", addr);
            Self::send_message(w, &message, "MINED_BLOCK").await;
        }
        // Update own chain length
        self.update_registry_chain_length().await;
    }

    pub async fn broadcast_transaction(&self, txn: Transaction) {
        println!("called broadcast txn");
        // Copy the peer list so the peer map is not held during network writes
        let writers: Vec<(String, Arc<Mutex<OwnedWriteHalf>>)> = self
            .peer_writers
            .lock()
            .await
            .iter()
            .map(|(a, w)| (a.clone(), w.clone()))
            .collect();
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_TRANSACTION,
            "transaction": txn,
        });
        for (addr, w) in writers {
            println!("Broadcasting Txn to {}", addr);
            Self::send_message(w, &message, "TRANSACTION").await;
        }
    }

    async fn connect_to_peers(self: Arc<Self>, peers: Vec<String>) {
        println!("Attempting to connect to {} peers", peers.len());
        for peer in peers {
            println!("Initiating connection to peer {}", peer);
            match timeout(Duration::from_secs(5), TcpStream::connect(&peer)).await {
                Ok(Ok(stream)) => {
                    println!("Successfully connected to peer {}", peer);
                    let addr = peer.clone();
                    let self_clone = self.clone();
                    tokio::spawn(async move {
                        self_clone.connect_socket(stream, addr).await;
                    });
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to connect to peer {}: {}", peer, e);
                }
                Err(_) => {
                    eprintln!("Timed out connecting to peer {}", peer);
                }
            }
        }
    }

    async fn connect_socket(self: Arc<Self>, socket: TcpStream, peer_addr: String) {
        let peer_addr_temp = format!("temp_{}", peer_addr);
        let (reader, writer) = socket.into_split();
        let writer = Arc::new(Mutex::new(writer));
        self.peer_writers
            .lock()
            .await
            .insert(peer_addr_temp.clone(), writer.clone());
        self.connected_peers
            .lock()
            .await
            .insert(peer_addr_temp.clone());
        let (chain_length, _last_ts) = self.get_chain_info().await;
        let genesis_block = {
            let tree = self.treechain.lock().await;
            tree.blocks.get_index(0).unwrap().1.clone()
        };
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_CONNECTION_INFO,
            "chain_length": chain_length,
            "last_block": genesis_block,
            "own_addr": self.own_addr.clone(),
        });
        println!(
            "Sending CONNECTION_INFO to {}: chain_length={}",
            peer_addr_temp, chain_length
        );
        Self::send_message(writer.clone(), &message, "CONNECTION_INFO").await;
        println!("Starting message handler for peer {}", peer_addr_temp);
        let self_clone = Arc::clone(&self);
        tokio::spawn(async move {
            self_clone
                .message_handler(reader, writer.clone(), peer_addr_temp)
                .await;
        });
    }

    async fn reset_sync_state(&self) {
        let mut sync_state = self.sync_state.lock().await;
        *sync_state = None;
        println!("Sync state reset");
    }

    async fn maybe_start_next_batch(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        println!("Called maybe_start_next_batch");
        let mut next_batch: Option<Vec<InvEntry>> = None;
        let mut after_empty_inv_was_full = false;
        {
            let mut st_opt = self.sync_state.lock().await;
            let st = match st_opt.as_mut() {
                Some(s) => s,
                None => {
                    println!("No sync state; requesting new inventory");
                    self.send_getblocks(writer.clone()).await;
                    return;
                }
            };
            println!(
                "Sync state: pending={}, inventories.len={}, inv_was_full={}",
                st.pending,
                st.inventories.len(),
                st.inv_was_full
            );
            if st.pending != 0 {
                println!("Current batch still in-flight; waiting for BLOCKs");
                return;
            }
            if st.inventories.is_empty() {
                after_empty_inv_was_full = st.inv_was_full;
                *st_opt = None;
                println!("Inventory exhausted; clearing sync state");
            } else {
                let take = std::cmp::min(GETDATA_LIMIT as usize, st.inventories.len());
                let rest = st.inventories.split_off(take);
                let chunk = std::mem::replace(&mut st.inventories, rest);
                st.pending = chunk.len();
                next_batch = Some(chunk);
                println!(
                    "Preparing next batch of {} entries",
                    next_batch.as_ref().unwrap().len()
                );
            }
        }
        if let Some(batch) = next_batch {
            self.send_getdatamessage(writer.clone(), batch).await;
        } else if after_empty_inv_was_full {
            println!("Inventory exhausted but was full; requesting next batch via GETBLOCKS");
            self.send_getblocks(writer.clone()).await;
        } else {
            let mut ibd = self.ibd_or_online_state.lock().await;
            *ibd = false;
            println!("✅ Finished syncing all available blocks from best peer");
            drop(ibd);
            self.update_reg_cl_precise().await;
            self.send_get_pqp(writer.clone()).await;
            self.send_get_txnpool(writer.clone()).await;
        }
    }

    async fn on_block_delivered(&self, writer: Arc<Mutex<OwnedWriteHalf>>) {
        println!("on_block_delivered called");
        let make_post_batch_call: bool;
        {
            let mut st_opt = self.sync_state.lock().await;
            let st = match st_opt.as_mut() {
                Some(s) => {
                    println!(
                        "Before decrement: pending={}, inventories.len={}",
                        s.pending,
                        s.inventories.len()
                    );
                    s
                }
                None => {
                    println!("No sync state; ignoring block delivery");
                    return;
                }
            };
            if st.pending == 0 {
                println!("No pending blocks expected; ignoring");
                return;
            }
            st.pending -= 1;
            println!("After decrement: pending={}", st.pending);
            make_post_batch_call = st.pending == 0;
        }
        if make_post_batch_call {
            let treechain = self.treechain.lock().await;
            let pqp = self.pqp.lock().await;
            if !treechain.is_valid_pqp(&pqp) {
                println!("❌ Invalid pqp detected after syncing a batch");
                self.reset_sync_state().await;
                return;
            }
            if !treechain.is_valid_tree(&pqp) {
                println!("❌ Invalid tree detected after syncing a batch");
                self.reset_sync_state().await;
                return;
            }
            drop(treechain);
            drop(pqp);
            println!("Batch complete, calling maybe_start_next_batch");
            self.maybe_start_next_batch(writer).await;
        }
    }

    async fn get_current_best_peer(&self) -> Option<(String, u64, Option<u64>)> {
        let peer_lengths = self.peer_lengths.lock().await;
        let connected_peers = self.connected_peers.lock().await;
        let best_peer = peer_lengths
            .iter()
            .filter(|(addr, _)| connected_peers.contains(*addr))
            .max_by_key(|(_, (len, _))| *len);
        best_peer.map(|(addr, (len, ts))| (addr.clone(), *len, Some(*ts)))
    }

    pub async fn get_best_peer(
        &self,
        writer: Arc<Mutex<OwnedWriteHalf>>,
    ) -> Option<(String, u64, Option<u64>)> {
        let message = serde_json::json!({
            "type": MESSAGE_TYPE_GET_PEER_LIST,
        });
        Self::send_message(writer.clone(), &message, "GET_PEER_LIST").await;

        let peer_lengths = self.peer_lengths.lock().await;
        let connected_peers = self.connected_peers.lock().await;
        let best_peer = peer_lengths
            .iter()
            .filter(|(addr, _)| connected_peers.contains(*addr))
            .max_by_key(|(_, (len, _))| *len);
        best_peer.map(|(addr, (len, ts))| (addr.clone(), *len, Some(*ts)))
    }

    async fn message_handler(
        self: Arc<Self>,
        mut reader: OwnedReadHalf,
        writer: Arc<Mutex<OwnedWriteHalf>>,
        peer_addr: String,
    ) {
        let mut buffer = vec![0u8; 8192];
        let mut received_data = Vec::new();
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    println!("Connection closed by peer: {}", peer_addr);
                    self.remove_writer(&peer_addr).await;
                    break;
                }
                Ok(n) => {
                    println!("Read {} bytes from {}", n, peer_addr);
                    received_data.extend_from_slice(&buffer[..n]);
                    while let Some((msg, remaining)) = Self::parse_message(&received_data) {
                        received_data = remaining;
                        println!(
                            "Processing message from {}: {:?}",
                            peer_addr,
                            msg.get("type")
                        );
                        match serde_json::from_value::<Value>(msg.clone()) {
                            Ok(data) => {
                                let msg_type =
                                    data.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                if self.isolated_now()
                                    && matches!(
                                        msg_type,
                                        MESSAGE_TYPE_MINEDBLOCK
                                            | MESSAGE_TYPE_BLOCK
                                            | MESSAGE_TYPE_INVMESSAGE
                                            | MESSAGE_TYPE_TREE_STATUS
                                            | MESSAGE_TYPE_TREE
                                    )
                                {
                                    println!("TEST_ISOLATE: dropping {} from {}", msg_type, peer_addr);
                                    continue;
                                }
                                match msg_type {
                                    MESSAGE_TYPE_TREE_STATUS => {
                                        let count =
                                            data.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                                        let digest = data
                                            .get("digest")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let addr = data
                                            .get("own_addr")
                                            .and_then(|v| v.as_str())
                                            .map(String::from)
                                            .unwrap_or_else(|| peer_addr.clone());
                                        self.peer_tree_status.lock().await.insert(
                                            addr,
                                            PeerTreeStatus {
                                                count,
                                                digest,
                                                received_at: Instant::now(),
                                                writer: writer.clone(),
                                            },
                                        );
                                    }
                                    MESSAGE_TYPE_GET_TREE => {
                                        // copy the blocks, then release the tree before sending
                                        let blocks: Vec<Block> = {
                                            let tree = self.treechain.lock().await;
                                            tree.blocks
                                                .values()
                                                .filter(|b| !b.position.is_empty())
                                                .cloned()
                                                .collect()
                                        };
                                        println!(
                                            "Sending our tree ({} blocks) to {}",
                                            blocks.len(),
                                            peer_addr
                                        );
                                        let message = serde_json::json!({
                                            "type": MESSAGE_TYPE_TREE,
                                            "blocks": blocks,
                                        });
                                        Self::send_message(writer.clone(), &message, "TREE").await;
                                    }
                                    MESSAGE_TYPE_TREE => {
                                        let blocks: Vec<Block> = data
                                            .get("blocks")
                                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                                            .unwrap_or_default();
                                        println!(
                                            "Received a tree of {} blocks from {}",
                                            blocks.len(),
                                            peer_addr
                                        );
                                        // replaying takes a while: do it off this connection's loop
                                        let server = self.clone();
                                        let from = writer.clone();
                                        tokio::spawn(async move {
                                            server.switch_to_tree(blocks, from).await;
                                        });
                                    }
                                    MESSAGE_TYPE_CONNECTION_INFO => {
                                        let chain_length_remote = data
                                            .get("chain_length")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        let remote_last_ts = data
                                            .get("last_block")
                                            .and_then(|lb| lb.get("timestamp"))
                                            .and_then(|t| t.as_u64());
                                        let own_addr = data
                                            .get("own_addr")
                                            .and_then(|v| v.as_str())
                                            .map(String::from)
                                            .unwrap_or_default();
                                        println!(
                                            "Received CONNECTION_INFO from {} with own_addr: {}",
                                            peer_addr, own_addr
                                        );
                                        if !own_addr.is_empty() && own_addr != peer_addr {
                                            let mut peer_writers = self.peer_writers.lock().await;
                                            let mut connected_peers =
                                                self.connected_peers.lock().await;
                                            if let Some(w) = peer_writers.remove(&peer_addr) {
                                                peer_writers.insert(own_addr.clone(), w);
                                            }
                                            if connected_peers.remove(&peer_addr) {
                                                connected_peers.insert(own_addr.clone());
                                            }
                                            println!("Updated peer_addr to {}", own_addr);
                                        }
                                        let (chain_length_local, local_last_ts) =
                                            self.get_chain_info().await;
                                        println!(
                                            "CONNECTION_INFO from {}: remote_len={}, local_len={}",
                                            own_addr, chain_length_remote, chain_length_local
                                        );
                                        let mut peer_lengths = self.peer_lengths.lock().await;
                                        peer_lengths.insert(
                                            own_addr.clone(),
                                            (chain_length_remote, remote_last_ts.unwrap_or(0)),
                                        );
                                        drop(peer_lengths);
                                        if chain_length_remote > chain_length_local {
                                            let remote_ahead = match (remote_last_ts, local_last_ts)
                                            {
                                                (Some(rts), Some(lts)) => {
                                                    rts.saturating_sub(lts) > DAY_MS
                                                }
                                                (Some(_), None) => true,
                                                _ => false,
                                            };
                                            if remote_ahead {
                                                println!(
                                                    "Remote ahead by >24h; reinitializing to genesis"
                                                );
                                                let mut treechain = self.treechain.lock().await;
                                                let genesis = treechain
                                                    .blocks
                                                    .get_index(0)
                                                    .unwrap()
                                                    .1
                                                    .clone();
                                                let ghash = genesis.hash.clone();
                                                treechain.blocks.clear();
                                                treechain.children_map.clear();
                                                treechain.blocks.insert(ghash.clone(), genesis);
                                                treechain.children_map.insert(ghash, vec![]);
                                                drop(treechain);
                                                let mut pqp = self.pqp.lock().await;
                                                *pqp = PQP::new();
                                                drop(pqp);
                                                {
                                                    let mut chain_length =
                                                        self.chain_length.lock().unwrap();
                                                    *chain_length = 1;
                                                    drop(chain_length);
                                                }
                                                self.update_registry_chain_length().await;
                                            }
                                            let mut ibd = self.ibd_or_online_state.lock().await;
                                            *ibd = true;
                                            drop(ibd);
                                            println!(
                                                "Sending GETBLOCKS to {} with longer chain",
                                                own_addr
                                            );
                                            self.send_getblocks(writer.clone()).await;
                                        } else {
                                            println!(
                                                "Peer {} chain not longer; no sync needed",
                                                own_addr
                                            );
                                        }
                                    }
                                    MESSAGE_TYPE_GETBLOCKS => {
                                        println!("Received GETBLOCKS from {}", peer_addr);
                                        let block_locator: Vec<String> = data
                                            .get("block_locator")
                                            .and_then(|v| v.as_array())
                                            .map(|arr| {
                                                arr.iter()
                                                    .filter_map(|v| v.as_str().map(String::from))
                                                    .collect()
                                            })
                                            .unwrap_or_default();
                                        self.send_invmessages(writer.clone(), block_locator).await;
                                    }
                                    MESSAGE_TYPE_INVMESSAGE => {
                                        println!(
                                            "Received INVMESSAGE from {} with {} entries",
                                            peer_addr,
                                            data.get("inventories")
                                                .and_then(|v| v.as_array())
                                                .map(|v| v.len())
                                                .unwrap_or(0)
                                        );
                                        let inventories: Vec<InvEntry> = data
                                            .get("inventories")
                                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                                            .unwrap_or_default();
                                        let mut st_opt = self.sync_state.lock().await;
                                        if inventories.clone().is_empty() {
                                            if let Some(st) = st_opt.as_mut() {
                                                if st.inv_was_full {
                                                    println!(
                                                        "Received empty INVMESSAGE from {}, but was full; requesting more",
                                                        peer_addr
                                                    );
                                                    drop(st_opt);
                                                    self.send_getblocks(writer.clone()).await;
                                                    continue;
                                                } else {
                                                    println!(
                                                        "Received empty INVMESSAGE; sync complete for {}",
                                                        peer_addr
                                                    );
                                                    *st_opt = None;
                                                    let mut ibd =
                                                        self.ibd_or_online_state.lock().await;
                                                    *ibd = false;
                                                    drop(ibd);
                                                    self.send_pqp_response(writer.clone()).await;
                                                    continue;
                                                }
                                            } else {
                                                println!(
                                                    "Received empty INVMESSAGE with no sync state; ignoring"
                                                );
                                                continue;
                                            }
                                        }
                                        let should_ignore = st_opt
                                            .as_ref()
                                            .map(|st| st.pending > 0 || !st.inventories.is_empty())
                                            .unwrap_or(false);
                                        if should_ignore {
                                            println!(
                                                "Currently syncing another window; ignoring new INVMESSAGE"
                                            );
                                            continue;
                                        }
                                        *st_opt = Some(SyncState {
                                            pending: 0,
                                            inventories: inventories.clone(),
                                            inv_was_full: inventories.len()
                                                >= INVMESSAGE_LIMIT as usize,
                                        });
                                        drop(st_opt);
                                        self.maybe_start_next_batch(writer.clone()).await;
                                    }
                                    MESSAGE_TYPE_GETDATA => {
                                        println!(
                                            "Received GETDATA from {} with {} entries",
                                            peer_addr,
                                            data.get("hashes")
                                                .and_then(|v| v.as_array())
                                                .map(|v| v.len())
                                                .unwrap_or(0)
                                        );
                                        let hashes: Vec<InvEntry> = data
                                            .get("hashes")
                                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                                            .unwrap_or_default();
                                        // Copy the requested blocks, then release the tree lock
                                        // before sending them over the network
                                        let requested: Vec<Block> = {
                                            let treechain = self.treechain.lock().await;
                                            hashes
                                                .iter()
                                                .filter_map(|inv| {
                                                    treechain.get_block(&inv.block_hash).cloned()
                                                })
                                                .collect()
                                        };
                                        for block in requested {
                                            let message = serde_json::json!({
                                                "type": MESSAGE_TYPE_BLOCK,
                                                "block": block,
                                            });
                                            Self::send_message(writer.clone(), &message, "BLOCK")
                                                .await;
                                        }
                                    }
                                    MESSAGE_TYPE_BLOCK => {
                                        if let Some(block_val) = data.get("block") {
                                            let block: Block =
                                                match serde_json::from_value(block_val.clone()) {
                                                    Ok(b) => b,
                                                    Err(e) => {
                                                        println!(
                                                            "❌ Failed to deserialize BLOCK: {}",
                                                            e
                                                        );
                                                        continue;
                                                    }
                                                };
                                            println!(
                                                "Received BLOCK {} from {}",
                                                block.hash, peer_addr
                                            );
                                            let mut treechain = self.treechain.lock().await;
                                            let mut pqp = self.pqp.lock().await;
                                            let mut utxo_set = self.utxo_set.lock().await;
                                            let mut txn_pool = self.txn_pool.lock().await;
                                            let pqp_entry =
                                                TreeChain::parent_queue_entry_from_block(&block);
                                            let pool_before = pqp.pool.clone();
                                            pqp.add_entry_to_pqp_while_downloading(
                                                pqp_entry.clone(),
                                                &treechain,
                                            );
                                            //this functionality to be added here
                                            let pqp_len = pqp.pool.len();
                                            let exist = pqp
                                                .pool
                                                .iter()
                                                .rev()
                                                .take(pqp_len)
                                                .any(|e| e.block_hash == pqp_entry.block_hash);
                                            if exist {
                                                let block_exists = treechain.get_block(&block.hash);
                                                let mut existing: bool = false;
                                                if block_exists != None {
                                                    existing = true;
                                                }
                                                if treechain.verify_and_add_block_to_tree(
                                                    &block,
                                                    &mut utxo_set,
                                                    &mut txn_pool,
                                                ) {
                                                    println!(
                                                        "✅ Block {} added successfully",
                                                        block.hash
                                                    );
                                                    {
                                                        let mut chain_length =
                                                            self.chain_length.lock().unwrap();
                                                        *chain_length = treechain.count as u64;
                                                        drop(chain_length);
                                                    }
                                                    drop(treechain);
                                                    drop(pqp);
                                                    drop(utxo_set);
                                                    drop(txn_pool);
                                                    if !existing {
                                                        self.update_registry_chain_length().await;
                                                    }
                                                    self.on_block_delivered(writer.clone()).await;
                                                } else {
                                                    println!(
                                                        "❌ Failed to add block {} (duplicate or invalid)",
                                                        block.hash
                                                    );
                                                    // undo the entry and any parent it retired
                                                    pqp.pool = pool_before;
                                                    drop(treechain);
                                                    drop(pqp);
                                                    drop(utxo_set);
                                                    drop(txn_pool);
                                                    self.on_block_delivered(writer.clone()).await;
                                                }
                                            } else {
                                                println!(
                                                    "❌ pqp entry of the block {} is not added while getting blocks",
                                                    pqp_entry.block_hash
                                                );
                                            }
                                        }
                                    }
                                    MESSAGE_TYPE_MINEDBLOCK => {
                                        let syncing = {
                                            let sync_state = self.sync_state.lock().await;
                                            matches!(&*sync_state, Some(st) if st.pending > 0 || !st.inventories.is_empty())
                                        };
                                        if syncing {
                                            // keep it for after the download instead of dropping it:
                                            // the monitor retries pending blocks every second
                                            if let Some(block) = data
                                                .get("block")
                                                .and_then(|v| serde_json::from_value::<Block>(v.clone()).ok())
                                            {
                                                println!(
                                                    "Queuing MINED_BLOCK {} from {} while downloading",
                                                    block.hash, peer_addr
                                                );
                                                self.pending_blocks
                                                    .lock()
                                                    .await
                                                    .insert(block.hash.clone(), block);
                                            }
                                            continue;
                                        }

                                        let block_hash_opt = data
                                            .get("block")
                                            .and_then(|v| v.get("hash"))
                                            .and_then(|v| v.as_str());
                                        if let Some(block_hash) = block_hash_opt {
                                            let mut processed =
                                                self.recently_processed_blocks.lock().await;
                                            if processed.iter().any(|h| h == block_hash) {
                                                println!(
                                                    "Duplicate MINED_BLOCK {} from {}; skipping",
                                                    block_hash, peer_addr
                                                );
                                                continue;
                                            }
                                            processed.push_back(block_hash.to_string());
                                            if processed.len() > 5 {
                                                processed.pop_front();
                                            }
                                            drop(processed);
                                        } else {
                                            println!(
                                                "❌ No hash in MINED_BLOCK from {}",
                                                peer_addr
                                            );
                                            continue;
                                        }
                                        if let Some(block_val) = data.get("block") {
                                            let block: Block = match serde_json::from_value(
                                                block_val.clone(),
                                            ) {
                                                Ok(b) => b,
                                                Err(e) => {
                                                    println!(
                                                        "❌ Failed to deserialize MINED_BLOCK: {}",
                                                        e
                                                    );
                                                    continue;
                                                }
                                            };
                                            println!(
                                                "Received MINED_BLOCK {} from {}",
                                                block.hash, peer_addr
                                            );
                                            {
                                                let treechain = self.treechain.lock().await;
                                                if let Some((_, existing_block)) = treechain
                                                    .blocks
                                                    .get_index(block.pqp_entry.queue_index as usize)
                                                {
                                                    if existing_block.hash == block.hash {
                                                        println!(
                                                            "Block {} already exists, ignoring",
                                                            block.hash
                                                        );
                                                        drop(treechain);
                                                        continue;
                                                    }
                                                }
                                                drop(treechain);
                                            }

                                            // Update peer_lengths
                                            let mut peer_lengths = self.peer_lengths.lock().await;
                                            if let Some((_, ts)) = peer_lengths.get_mut(&peer_addr)
                                            {
                                                *ts = block.timestamp as u64;
                                                *peer_lengths.get_mut(&peer_addr).unwrap() =
                                                    (*ts, block.timestamp as u64);
                                            }
                                            drop(peer_lengths);

                                            // Check mining conflict
                                            let mut treechain = self.treechain.lock().await;
                                            let mut pqp = self.pqp.lock().await;
                                            let cp = pqp.current_parent();
                                            let np = pqp.next_parent();
                                            if let (Some(cp), Some(np)) = (cp, np) {
                                                let mut cp_indexes: Vec<u32> = Vec::new();
                                                let mut np_indexes: Vec<u32> = Vec::new();
                                                for i in 1..CHILDREN + 1 {
                                                    let a = TreeChain::child_index(
                                                        cp.queue_index,
                                                        i as u32,
                                                    );
                                                    let b = TreeChain::child_index(
                                                        np.queue_index,
                                                        i as u32,
                                                    );
                                                    cp_indexes.push(a);
                                                    np_indexes.push(b);
                                                }
                                                let target_guard =
                                                    self.current_mining_position.lock().unwrap();
                                                if let Some(target) = target_guard.as_ref() {
                                                    if block.position == *target || {
                                                        let target_parts: Vec<&str> =
                                                            target.split('.').collect();
                                                        let block_parts: Vec<&str> =
                                                            block.position.split('.').collect();
                                                        if !cp_indexes
                                                            .contains(&block.pqp_entry.queue_index)
                                                            && np_indexes.contains(
                                                                &block.pqp_entry.queue_index,
                                                            )
                                                        {
                                                            true
                                                        } else if target_parts.len()
                                                            < block_parts.len()
                                                        {
                                                            true
                                                        } else {
                                                            false
                                                        }
                                                    } {
                                                        println!(
                                                            "Received MINED_BLOCK with position {} as current mining target, aborting current mine",
                                                            target
                                                        );
                                                        *self.abort_mining.lock().unwrap() = true;
                                                    }
                                                }
                                                drop(target_guard);
                                            }

                                            let mut utxo_set = self.utxo_set.lock().await;
                                            let mut txn_pool = self.txn_pool.lock().await;
                                            let pqp_entry =
                                                TreeChain::parent_queue_entry_from_block(&block);
                                            let pool_before = pqp.pool.clone();
                                            pqp.add_entry_to_pqp(pqp_entry.clone(), &treechain);
                                            let pqp_len = pqp.pool.len();
                                            let exist = pqp
                                                .pool
                                                .iter()
                                                .rev()
                                                .take(pqp_len)
                                                .any(|e| e.block_hash == pqp_entry.block_hash);
                                            if exist {
                                                if treechain.verify_and_add_block_to_tree(
                                                    &block.clone(),
                                                    &mut utxo_set,
                                                    &mut txn_pool,
                                                ) {
                                                    println!(
                                                        "✅ MINED_BLOCK {} added successfully",
                                                        block.hash
                                                    );
                                                    {
                                                        let mut chain_length =
                                                            self.chain_length.lock().unwrap();
                                                        *chain_length = treechain.count as u64;
                                                        drop(chain_length);
                                                    }
                                                    // Release the state locks before network I/O so a
                                                    // slow peer cannot stall block processing on this node
                                                    drop(treechain);
                                                    drop(pqp);
                                                    drop(utxo_set);
                                                    drop(txn_pool);
                                                    // Broadcast the mined block to all connected peers
                                                    let message = serde_json::json!({
                                                        "type": MESSAGE_TYPE_MINEDBLOCK,
                                                        "block": block.clone(),
                                                    });
                                                    let targets: Vec<(String, Arc<Mutex<OwnedWriteHalf>>)> = self
                                                        .peer_writers
                                                        .lock()
                                                        .await
                                                        .iter()
                                                        .map(|(a, w)| (a.clone(), w.clone()))
                                                        .collect();
                                                    for (addr, w) in targets {
                                                        println!(
                                                            "Broadcasting MINED_BLOCK to {}",
                                                            addr
                                                        );
                                                        Self::send_message(
                                                            w,
                                                            &message,
                                                            "MINED_BLOCK",
                                                        )
                                                        .await;
                                                    }
                                                    self.update_registry_chain_length().await;
                                                    // Update peer_lengths and trigger sync if needed
                                                    let (local_len, _) =
                                                        self.get_chain_info().await;
                                                    let mut peer_lengths =
                                                        self.peer_lengths.lock().await;
                                                    if let Some((peer_len, _)) =
                                                        peer_lengths.get_mut(&peer_addr)
                                                    {
                                                        *peer_len = local_len + 1;
                                                    }
                                                    drop(peer_lengths);
                                                    // if let Some((best_addr, best_len, _)) =
                                                    //     self.get_current_best_peer().await
                                                    // {
                                                    //     if best_len > local_len {
                                                    //         println!(
                                                    //             "Best peer {} has longer chain ({} vs {}); triggering sync",
                                                    //             best_addr, best_len, local_len
                                                    //         );
                                                    //         let writers =
                                                    //             self.peer_writers.lock().await;
                                                    //         if let Some(w) = writers.get(&best_addr)
                                                    //         {
                                                    //             self.send_getblocks(w.clone())
                                                    //                 .await;
                                                    //         }
                                                    //     }
                                                    // }
                                                } else {
                                                    drop(treechain);
                                                    drop(utxo_set);
                                                    drop(txn_pool);
                                                    println!(
                                                        "❌ Failed to add MINED_BLOCK {} (duplicate or invalid)",
                                                        block.hash
                                                    );
                                                    // undo the entry and any parent it retired
                                                    pqp.pool = pool_before;
                                                    drop(pqp);
                                                }
                                            } else {
                                                println!(
                                                    "ParentQueueEntry of MINED_BLOCK {} not added; queuing block",
                                                    block.hash
                                                );
                                                let expected_prev_pqp_commit = pqp
                                                    .get_prev_pqp_commitment(
                                                        block.align,
                                                        &treechain,
                                                    );
                                                let block_at_that_index_empty =
                                                    treechain.blocks.iter().any(|(_h, b)| {
                                                        b.pqp_entry.queue_index
                                                            == block.pqp_entry.queue_index
                                                            && b.position.is_empty()
                                                    });
                                                let request_blocks = block
                                                    .pqp_entry
                                                    .prev_pqp_commitment
                                                    != expected_prev_pqp_commit
                                                    || block_at_that_index_empty;
                                                // Release every state lock before touching other locks or
                                                // the network (get_chain_info below re-locks the tree)
                                                drop(treechain);
                                                drop(pqp);
                                                drop(utxo_set);
                                                drop(txn_pool);
                                                if request_blocks {
                                                    println!(
                                                        "Missing parent for MINED_BLOCK {}; queuing and requesting blocks",
                                                        block.hash
                                                    );
                                                } else {
                                                    println!(
                                                        "Queuing MINED_BLOCK {} due to invalid PQP entry",
                                                        block.hash
                                                    );
                                                }
                                                self.pending_blocks
                                                    .lock()
                                                    .await
                                                    .insert(block.hash.clone(), block.clone());
                                                if request_blocks {
                                                    self.send_getblocks(writer.clone()).await;
                                                }

                                                let (local_len, _) = self.get_chain_info().await;
                                                if let Some((best_addr, best_len, _)) =
                                                    self.get_best_peer(writer.clone()).await
                                                {
                                                    if best_len > local_len {
                                                        println!(
                                                            "MINED_BLOCK queue_index invalid and best peer {} has longer chain ({} vs {}); requesting its tree",
                                                            best_addr, best_len, local_len
                                                        );
                                                        // switch through the validated path
                                                        // (switch_to_tree), never by resetting
                                                        // the tree in place
                                                        let best_writer = self
                                                            .peer_writers
                                                            .lock()
                                                            .await
                                                            .get(&best_addr)
                                                            .cloned();
                                                        if let Some(w) = best_writer {
                                                            let message = serde_json::json!({
                                                                "type": MESSAGE_TYPE_GET_TREE
                                                            });
                                                            Self::send_message(w, &message, "GET_TREE")
                                                                .await;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    MESSAGE_TYPE_GET_PQP => {
                                        println!("Received GET_PQP from {}", peer_addr);
                                        self.send_pqp_response(writer.clone()).await;
                                    }
                                    MESSAGE_TYPE_PQP_RESPONSE => {
                                        let remote_pool: Vec<ParentQueueEntry> =
                                            serde_json::from_value(
                                                data.get("pool").cloned().unwrap_or_default(),
                                            )
                                            .unwrap_or_default();
                                        println!(
                                            "Received PQP_RESPONSE from {} with {} entries",
                                            peer_addr,
                                            remote_pool.len()
                                        );
                                        let local_pqp = self.pqp.lock().await;
                                        let local_pool = local_pqp.pool.clone();
                                        drop(local_pqp);
                                        let pools_match = local_pool.len() == remote_pool.len()
                                            && local_pool.iter().zip(remote_pool.iter()).all(
                                                |(local, remote)| {
                                                    local.queue_index == remote.queue_index
                                                        && local.block_hash == remote.block_hash
                                                        && local.prev_pqp_commitment
                                                            == remote.prev_pqp_commitment
                                                },
                                            );
                                        if pools_match {
                                            println!("✅ Local and remote PQP pools match");
                                        } else {
                                            println!("❌ Local and remote PQP pools do not match");
                                        }
                                        let treechain = self.treechain.lock().await;
                                        let pqp = self.pqp.lock().await;
                                        let is_valid = treechain.is_valid_pqp(&pqp);
                                        if is_valid {
                                            println!("✅ Own PQP is valid after full sync");
                                        } else {
                                            println!("❌ Own PQP is invalid after full sync");
                                        }
                                    }
                                    MESSAGE_TYPE_TRANSACTION => {
                                        println!("received broadcasted txn");
                                        if let Some(txn_val) = data.get("transaction") {
                                            let txn: Transaction = match serde_json::from_value(
                                                txn_val.clone(),
                                            ) {
                                                Ok(b) => b,
                                                Err(e) => {
                                                    println!(
                                                        "❌ Failed to deserialize Broadcasted Transaction: {}",
                                                        e
                                                    );
                                                    continue;
                                                }
                                            };
                                            {
                                                // Lock order used everywhere: treechain -> pqp -> utxo_set -> txn_pool
                                                let treechain = self.treechain.lock().await;
                                                let utxo_set = self.utxo_set.lock().await;
                                                let mut txn_pool = self.txn_pool.lock().await;
                                                let _ = txn_pool
                                                    .add_transaction(txn, &utxo_set, &treechain);
                                                drop(txn_pool);
                                                drop(utxo_set);
                                                drop(treechain);
                                            }
                                        }
                                    }
                                    MESSAGE_TYPE_GET_TRANSACTION_POOL => {
                                        println!(
                                            "Received GET_TRANSACTION_POOL from {}",
                                            peer_addr
                                        );
                                        let txn_pool = self.txn_pool.lock().await;
                                        let transactions: Vec<Transaction> = txn_pool
                                            .pool
                                            .values()
                                            .map(|entry| entry.tx.clone())
                                            .collect();
                                        drop(txn_pool);

                                        for txn in transactions.clone() {
                                            let message = serde_json::json!({
                                                "type": MESSAGE_TYPE_TRANSACTION,
                                                "transaction": txn,
                                            });
                                            Self::send_message(
                                                writer.clone(),
                                                &message,
                                                "TRANSACTION",
                                            )
                                            .await;
                                        }
                                        println!(
                                            "Sent {} transactions to {}",
                                            transactions.clone().len(),
                                            peer_addr
                                        );
                                    }

                                    _ => {
                                        println!("Other type from {}: {:?}", peer_addr, data);
                                    }
                                }
                            }
                            Err(e) => eprintln!("Invalid JSON from {}: {}", peer_addr, e),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Read error from {}: {}", peer_addr, e);
                    self.remove_writer(&peer_addr).await;
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
        let mut start_index = None;
        for locator in block_locator {
            if let Some((idx, (_hash, _))) = treechain
                .blocks
                .iter()
                .enumerate()
                .find(|(_, (hash, _))| *hash == &locator)
            {
                start_index = Some(idx + 1);
                break;
            }
        }
        let start = start_index.unwrap_or(0);
        let mut inventories: Vec<InvEntry> = treechain
            .blocks
            .iter()
            .skip(start)
            .filter(|(_, block)| !block.position.is_empty())
            .map(|(hash, block)| InvEntry {
                queue_index: block.pqp_entry.queue_index,
                block_hash: hash.clone(),
            })
            .take(INVMESSAGE_LIMIT as usize)
            .collect();
        // Release the tree lock before sending over the network
        drop(treechain);
        inventories.sort_by(|a, b| a.queue_index.cmp(&b.queue_index));
        if inventories.is_empty() {
            println!(
                "No blocks to send in INVMESSAGE (start_index: {:?})",
                start_index
            );
        } else {
            println!("Sending INVMESSAGE with {} entries", inventories.len());
            let message = serde_json::json!({
                "type": MESSAGE_TYPE_INVMESSAGE,
                "inventories": inventories
            });
            Self::send_message(writer, &message, "INVMESSAGE").await;
        }
    }

    async fn remove_writer(&self, addr: &str) {
        self.connected_peers.lock().await.remove(addr);
        self.peer_writers.lock().await.remove(addr);
        println!("Removed disconnected peer {}", addr);
    }

    pub fn start_p2p_server(
        p2p_port: String,
        _peers: String,
        _miner_address: String,
        treechain: Arc<Mutex<TreeChain>>,
        pqp: Arc<Mutex<PQP>>,
        utxo_set: Arc<Mutex<UtxoSet>>,
        txn_pool: Arc<Mutex<TransactionPool>>,
        current_mining_position: Arc<SyncMutex<Option<String>>>,
        abort_mining: Arc<SyncMutex<bool>>,
        chain_length: Arc<SyncMutex<u64>>,
    ) -> Arc<P2PServer> {
        let own_addr = format!("0.0.0.0:{}", p2p_port);
        // let own_addr = format!("172.30.255.27:{}", p2p_port); //for connecting to differnt device

        let server = Arc::new(P2PServer::new(
            treechain,
            pqp,
            utxo_set,
            txn_pool,
            vec![],
            current_mining_position,
            abort_mining,
            own_addr.clone(),
            chain_length,
        ));
        let server_clone = server.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                server_clone.clone().connect_to_registry(own_addr).await;
                server_clone
                    .listen(p2p_port.parse::<u16>().expect("Invalid port number"))
                    .await;
            })
        });
        server
    }

    async fn listen(self: Arc<Self>, port: u16) {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
            .await
            .unwrap();
        println!("Listening for peer to peer connections on 0.0.0.0:{}", port);
        loop {
            let (stream, addr) = listener.accept().await.unwrap();
            println!("New connection from {}", addr);
            let self_clone = self.clone();
            tokio::spawn(async move {
                self_clone.connect_socket(stream, addr.to_string()).await;
            });
        }
    }
}
