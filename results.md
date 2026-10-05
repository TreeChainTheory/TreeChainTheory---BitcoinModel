# Results: local 6-node runs of the TreeChainTheory PoW prototype

**What this is:** a real 25-minute run of the prototype, plus a 3-minute test that forces a fork. Six node processes and the ports server ran on **one MacBook Air (Apple M4, 10 cores, 16 GB, macOS 27.0)**, all on `localhost`. The Rust code mined and validated every block; nothing here is simulated.

**What this is not:** a distributed deployment. There was no emulated network delay and no adversary. Read it as **functional validation** of the prototype, not as evidence for throughput at scale (see Section 5).

- **Date:** 25-minute run on 2026-10-04/05, mining from 23:57:40 to 00:22:40 IST (1,500 s). Fork test on 2026-10-05, mining from 07:59:47 to 08:02:47 IST (180 s).
- **Code:** commit `1b91540` plus the changes in Section 1 (uncommitted at the time of the runs). 25-minute run: debug build (`cargo build`, Rust 1.97.0), the same kind of build the `.sh` script uses. Fork test: release build.
- **Config:** `CHILDREN = 3`, genesis `BITS = 1e1fffff`. For these runs `MINING_RATE = 15 s` and `EXPECTED_TIME = 150 s`, so the difficulty is retargeted every 30 blocks; the repository defaults are 100 s and 10,000 s (every 300 blocks). With the defaults a 25-minute run never reaches the first retarget, so the difficulty rule would not be exercised. All other rules are unchanged.
- **Raw data and scripts:** `experiments/local_6node_run/` (25-minute run) and `experiments/fork_choice_test/` (fork test).

## Summary

| Check | Result |
|---|---|
| Node processes alive at every 30 s snapshot | **306 / 306** (6 nodes × 51 snapshots) |
| Snapshots answered without an error | **306 / 306**; slowest `/get_blocks` reply 50 ms |
| Snapshots in which all six nodes held the same block set | **50 / 51**; at the other one (t = 1,022 s) node 1 had not yet received the newest block |
| Final trees | **identical on all six nodes**: 259 blocks (258 mined + genesis) |
| `/verify_tree` and `/verify_pqp` | valid on all six nodes (tree check re-run on the saved state after a fix to the check, see 3.2) |
| Difficulty retargets | **8**, and all 258 blocks carry exactly the bits the rule gives (recomputed offline) |
| "Bits mismatch" rejections, slot races, tree switches, panics | **0, 0, 0, 0** |
| Routing | **332 / 332** confirmed transactions were in the lane given by the locking scripts of their inputs, including the 86 with several inputs |
| Fork test (node 6 cut off for 60 s) | node 6 switched to the longer tree once; all six nodes identical from 153 s on and at the end (76 mined blocks); `/verify_tree` and `/verify_pqp` valid on all six |

---

## 1. Code changes since the previous results

The previous results (2026-09-22) came from an older version. Its two fixes, a single lock order in all handlers and the deterministic difficulty rule `TreeChain::bits_for_upcount`, are still in place. The protocol rules for slots, the PQP window, lane links and block counts are unchanged. These changes came after it:

1. **Routing by the hash of the locking script** (`src/wallet/transaction_pool.rs`, `src/treechain/treechain.rs`).

   | | Before | Now |
   |---|---|---|
   | Lane of a coin | last hex digit of the public key revealed when it is spent (multisig: last digit of the funding txid) | `(first 64 bits of SHA-256(locking script) % CHILDREN) + 1` |
   | When is it known | only when the coin is spent | as soon as the coin is created |
   | Which inputs are checked | only the first input | every input; a transaction whose inputs fall into different lanes is rejected |
   | Spread of owners over lanes | even only when `CHILDREN` divides 16 | even for any `CHILDREN` (see `experiments/bitcoin_lane_analysis/`) |

   `TransactionPool::lane_of_script` and `TransactionPool::tx_lane` compute the lane. It is stored with every mempool entry, miners select by it, and validators recompute it for every transaction of a block. `/wallet` reports the wallet's `lane`.
2. **Wallets in a chosen lane** (`src/wallet/wallet.rs`, `src/main.rs`). With the optional environment variable `WALLET_LANE=1..CHILDREN`, a node draws keys (`Wallet::new_in_lane`) until its address maps to that lane. Without it, the wallet key is random as before. The test driver uses it so that each lane holds two wallets (with random keys, five of six wallets once landed in lane 1).
3. **Longest-tree fork choice** (`src/treechain/treechain.rs`, `src/miner/p2p_server.rs`, `src/miner/routes.rs`). A tree with more blocks wins; between trees with the same number of blocks, the one with the smaller digest wins (`TreeChain::tree_digest`: SHA-256 over the block hashes in slot order), so every node picks the same tree.
   - Every 5 s each node sends `TREE_STATUS` (block count and digest) to its peers.
   - If a peer reports a better tree for 8 s, the node sends `GET_TREE` and receives `TREE` (all blocks).
   - `TreeChain::rebuild_from_blocks` replays the received blocks from genesis in slot order through the full validation. Only if that succeeds and the tree is still better does the node swap its tree, PQP and UTXO set.
   - Transactions from the node's blocks that the new tree lacks go back to the mempool. The miner pauses for 3 s and the node fetches any blocks it is still missing.
   - The old "reinitialize sync" path for an unexpected `MINED_BLOCK` now asks for the peer's tree instead of resetting the node's tree.
4. **Sync fixes** (`src/miner/p2p_server.rs`).
   - A `MINED_BLOCK` that arrives while the node is downloading blocks is queued as pending instead of dropped.
   - Pending blocks are dropped once they can never fit: already in the tree, slot taken, or parent retired.
5. **Tree check and parent queue fixes (2026-10-05, after the 25-minute run)**.
   - Coinbase maturity and timelocks are judged at the slot of the block that contains the transaction (`TransactionPool::validate_transaction_at`), not at the highest slot already in the tree. Whether a block is valid then no longer depends on which other blocks a node holds. The mempool still checks against the highest slot.
   - The miner leaves out of its template any transaction that spends a coinbase not yet mature at the template's slot (`TransactionPool::spends_immature_coinbase`).
   - Pending blocks are retried in slot order and join the tree only if the parent queue accepted them, as on every other path. Before, the retry path could add a block whose parent-queue entry had been rejected.
   - On every path, a block that fails validation restores the parent queue to its state before the block, so a rejected block can no longer retire a parent.
   - Section 3.2 and Section 4 explain how these issues showed up.
6. **Test hooks.** `TEST_ISOLATE=start:duration` (environment variable, off by default) makes a node drop incoming blocks and tree messages during that window, to force a fork. In the driver, `TESTBED_BUILD=release` selects the release build and `TESTBED_ISOLATE=node:start:duration` passes that window to one node.

Unit tests: 24 of 25 pass. `test_get_prev_pqp_only_genesis` fails regardless of these changes, because it still expects the genesis commitment from before commit `6726e5f`. New tests cover the routing rule, lane-aware wallets, the fork-choice comparison, rebuilding from genesis, and coinbase maturity. One ignored test, `verify_saved_tree`, runs `/verify_tree` offline on a node's saved blocks and PQP.

## 2. Setup

| Node | ALIGN (mining lane) | HTTP / P2P port | Wallet's transaction lane |
|---|---|---|---|
| 1 | 1 | 3001 / 5001 | 2 |
| 2 | 2 | 3002 / 5002 | 3 |
| 3 | 3 | 3003 / 5003 | 1 |
| 4 | 1 | 3004 / 5004 | 2 |
| 5 | 2 | 3005 / 5005 | 3 |
| 6 | 3 | 3006 / 5006 | 1 |

Each wallet's lane is set with `WALLET_LANE`: two wallets per lane, and never the lane the node itself mines, so every payment is mined by other nodes.

- **Nodes:** started as `starteverything_startmining.sh` does: the ports server, then six nodes 0.5 s apart, then `/start_mining` on each. `tmux` is not installed on this Mac, so the same commands ran as background processes. The frontends were skipped because they only display data. Every node had 5 peers before mining started.
- **Workload:** every 4 s, each wallet without an unconfirmed payment paid a random other wallet 0.1 to 1 coin with a 10,000-sat fee through `POST /create_txn`. These are real signed P2PKH transactions. Each wallet had at most one unconfirmed payment at a time, because the prototype's wallet always spends its oldest UTXOs and a second payment would double-spend the first.
- **Snapshots:** every 30 s the driver recorded each node's blocks, PQP size, mempool, peers, whether its process was alive, and how long it took to answer.
- **End of run:** after 1,500 s the driver called `/stop_mining` on every node and waited 30 s for in-flight blocks. It then took a final snapshot and saved each node's blocks, PQP, children map, wallet, UTXO set, mempool, peers, and the results of `/verify_tree` and `/verify_pqp`.

## 3. Results of the 25-minute run

### 3.1 No node stopped

- All six node processes ran from start to finish. At all 51 snapshots every process was alive and answered every request; the slowest `/get_blocks` reply took 50 ms.
- The node logs contain no panics, errors or timeouts. The frequent line "Data too short for message length: 0 bytes" is normal: the message parser prints it whenever its buffer is empty. All six nodes answered `/stop_mining` and the final dumps before the driver shut them down.
- Every node kept mining to the end. Each node's last block in the tree was mined at t = 1,403 / 1,490 / 1,501 / 1,483 / 1,496 / 1,492 s (nodes 1–6).

### 3.2 Consistency and validity

- **Snapshots:** all six nodes held the same block set at 50 of the 51 snapshots. At t = 1,022 s node 1 held 161 blocks and the other five held 162: the newest block had not reached node 1 yet. The block counts never differed by more than one.
- **Final trees:** identical on all six nodes, with 259 blocks (258 mined plus genesis).
- **Independent checks on the final tree (all passed):**
  - The slot rule `queue_index = 3 * parent_queue_index + align` holds for all 258 blocks.
  - Every `prev_pqp_commitment` equals the `pqp_commitment` of the nearest earlier block in the same lane.
- **Node checks:**
  - `/verify_pqp` returned "PQP is valid" on every node.
  - The `/verify_tree` call at the end of the run returned "Tree is invalid" on every node. The cause was a bug in the check, not in the tree.
    - The check replays the blocks in slot order and judged coinbase maturity against the highest slot already in the replayed tree.
    - When the replay reached a block that spends a coinbase, fewer of the higher slots were present than when the nodes had accepted that block live, so the spend looked immature.
  - After the fix (Section 1, item 5), the same check, run offline on each node's saved blocks and PQP, returns "Tree is valid" on all six nodes (`final/verify_tree_recheck.json`).
  - The fix does not change anything this run did:
    - No block was rejected for maturity during the run.
    - Every coinbase spend in the tree comes at least 13 slots after its coinbase (the rule requires 10).
    - The pending-block path changed in the same fix never ran (0 pending blocks).
- **Races, late blocks, switches:** 0 slot races, 0 blocks that pointed at a retired parent, 0 pending blocks, 0 tree requests, 0 tree switches.

### 3.3 Block production

| Metric | Value |
|---|---|
| Blocks mined (excluding genesis) | 258 (the last one at t = 1,501 s) |
| Overall rate | 10.3 blocks/min (target: 12/min, one block every 5 s) |
| Interval between blocks, any lane | mean 5.84 s, median 4.03 s |
| Blocks per lane (1 / 2 / 3) | 81 / 90 / 87 |
| Interval within a lane | mean 18.5 / 16.8 / 16.7 s, median 12.6 / 12.5 / 10.4 s (target 15 s) |
| Blocks in the tree per miner (nodes 1–6) | 43 / 48 / 47 / 38 / 42 / 40 |
| Mining restarts because a new block arrived | 207–218 per node |

Block rate in 5-minute windows:

| Window (s) | 0–300 | 300–600 | 600–900 | 900–1200 | 1200–1500 |
|---|---|---|---|---|---|
| Blocks | 25 | 41 | 71 | 56 | 63 |
| Seconds per block | 12.0 | 7.3 | 4.2 | 5.4 | 4.8 |

The first ten minutes ran at the genesis difficulty and the first retarget. **From t = 600 s the tree averaged 4.7 s per block (191 blocks in 900 s), against the 5 s target.**

### 3.4 Difficulty adjustment

The target is one block per lane every 15 s, so one block every 5 s across the three lanes.

How the rule works:
- Period *k* covers the blocks whose count is between 30*k* and 30*k* + 29. A block's count is the number of real blocks before its sibling group, plus its lane.
- Period *k*'s bits come from period *k*−1's bits, scaled by the time between the blocks with counts 30(*k*−1)−3 and 30*k*−3.
- That time is normalized to 30 block intervals and compared with 150 s. The factor is clamped to [¼, 4].

| Period | Counts | Bits | Difficulty vs genesis | Retarget window (counts, duration) | Target factor | Blocks | Mean interval |
|---|---|---|---|---|---|---|---|
| 0 | 0–29 | `1e1fffff` | 1.00 | (genesis bits) | – | 27 | 11.6 s |
| 1 | 30–59 | `1e45ed88` | 0.46 | 2–27, 273.2 s (327.8 s scaled to 30) | 2.19 | 31 | 8.3 s |
| 2 | 60–89 | `1e796bfa` | 0.26 | 27–57, 260.5 s | 1.74 | 29 | 4.1 s |
| 3 | 90–119 | `1e6ddd67` | 0.29 | 57–87, 135.7 s | 0.90 | 31 | 5.6 s |
| 4 | 120–149 | `1e7b10f3` | 0.26 | 87–117, 168.0 s | 1.12 | 30 | 3.4 s |
| 5 | 150–179 | `1e4c4e1e` | 0.42 | 117–147, 93.0 s | 0.62 | 28 | 5.4 s |
| 6 | 180–209 | `1e4b1a21` | 0.43 | 147–177, 147.6 s | 0.98 | 31 | 6.9 s |
| 7 | 210–239 | `1e5fb5e8` | 0.33 | 177–207, 191.2 s | 1.27 | 30 | 5.3 s |
| 8 | 240–269 | `1e5d39d6` | 0.34 | 207–237, 146.1 s | 0.97 | 21 (run ended) | 3.6 s |

"Difficulty vs genesis" is the genesis target divided by the period's target (below 1 means easier). The target factor is new target ÷ old target. Mean intervals are approximate: a period is defined by counts, not by time, so neighboring periods overlap by a few seconds.

- **Every node agreed on every retarget.** All 258 blocks carry exactly the bits that an offline re-implementation of `bits_for_upcount` computes from the final tree. No node logged a "Bits mismatch". All six nodes verified the same values for the first sibling group of each period (26 blocks).
- **The adjustment worked as intended.**
  - The genesis difficulty was too hard for six debug-build miners on this laptop (11.6 s per block), so the first two retargets made mining easier.
  - From period 2 on, the period means ranged from 3.4 to 6.9 s around the 5 s target.
  - A period has only 30 blocks, so its mean interval varies by about 18% by chance alone. Each retarget responds to the period before it, as in Bitcoin.
- **One brief template change, no disagreement:**
  - What happened: at the start of period 4, the two lane-3 miners (nodes 3 and 6) built a template for slot 1,296 while one block of the previous sibling group had not reached them yet. When it arrived, the block's count and the last block of the retarget window both changed. Both miners rebuilt the template with the agreed bits before finding a block, so no block ever carried the other value.
  - When it can happen: only for the first sibling group of a period.
  - How to avoid it: ending the window one sibling group earlier (count 30*k*−6) would remove it.
- **Noisy window measurement:** the window measures the time between two blocks in slot order. Siblings arrive in any order (57 of the 257 consecutive pairs in slot order have decreasing timestamps), so one window can be off by the gap between two siblings. Using the median timestamp of a sibling group would smooth this.

### 3.5 Tree shape and empty slots

| Metric | Value |
|---|---|
| Closed sibling groups (parents already retired) | 131 |
| Slots in those groups | 393 |
| Filled / empty | 256 / 137, a **fill rate of 65.1%** |
| Children per closed parent | 1 child: 39 parents, 2 children: 59, 3 children: 33 (mean 1.95) |
| Tree depth | 8 levels |
| Highest slot index | **5,739**, so each node stores 5,481 placeholder records for 258 real blocks |

A Monte Carlo of the window rule alone (`fill_rate_montecarlo.py`: equal hash power per lane, zero delay) predicts a 63.0% fill rate for N = 3.
- **Compared with the prediction:** 65.1% is 1.0 standard deviations above it (one standard deviation is 2.1 points for 131 groups). The children per parent match too: 30% / 45% / 25% of the parents got one / two / three children, against 33% / 45% / 22% in the model.
- **No finder head start this time:**
  - In the previous run the node that found a block also found the next one 23.6% of the time, against 16.7% for six equal miners, and that lowered the fill rate to 59.3%.
  - In this run the same node found the next block exactly 16.7% of the time, and the other miner of the same lane 19.1%.
  - With this run's measured head start, `simulate_with_finder_advantage` predicts 62.9%, the same as the plain model.
  - Numbers: `analysis.json` → `next_block`.

The conclusions for the paper stay the same:
- Empty slots come from the window rule itself, not from the network.
- They do not waste mining work, because a miner whose parent retires simply moves on to the next parent.
- Slot indices grow much faster than the block count: slot 5,739 after 258 blocks. The prototype stores a placeholder for every empty slot and uses a `u32` index. The paper recommends sparse slot storage and 64-bit indices.

### 3.6 Age of parent references (checks the paper's Observation 1)

- Measured in counts (position in slot order), the median of parent count ÷ block count is **0.50**, in the whole run and in its second half. The prediction 1/(fill rate × N) = 1/(0.651 × 3) is **0.51**.
- Measured in time, the median of parent time ÷ block time is 0.61 (also 0.61 in the second half). It is higher because the first blocks came slowly at the genesis difficulty, so the early counts cover more time.
- So a recent block points to a parent created about halfway back in the tree's history. At first only the block's own lane links protect it, as the paper argues.

### 3.7 Transactions

| Metric | Value |
|---|---|
| Submission attempts | 497 |
| Accepted into a mempool | 334 |
| Confirmed in blocks | 332; the other 2 were submitted at t = 1,485 s and sat in every node's mempool at the end (the same 2 on all nodes) |
| Confirmed per lane (1 / 2 / 3) | 102 / 117 / 113 (two sending wallets per lane) |
| Routing check | **332 / 332** confirmed transactions were in a block of the lane given by the locking scripts of all their inputs, including the 86 with several inputs |
| Confirmation latency (submission → block) | median 13.9 s, mean 17.3 s, 90th percentile 36.1 s |
| Confirmed throughput | 13.3 tx/min (limited by the workload; see below) |

Rejected submissions, all at the start of the run:

| Count | Reason | When |
|---|---|---|
| 97 | "insufficient funds": the wallet had no coins yet | all before t = 133 s |
| 66 | "coinbase UTXO not mature (requires 10 confirmations)" | all before t = 169 s |

After t = 169 s every submission was accepted. No transaction was rejected for its lane, and no block was rejected because of a transaction. Blocks carried 1.29 user transactions on average (at most 2), against a capacity of about 11 (10,240 weight units). The throughput above therefore reflects the six-wallet workload, not what the tree can carry.

## 4. Fork-choice test

The 25-minute run had no fork, so the fork choice never ran. To exercise it, the same six nodes, workload and test timing ran for 180 s with the release build, and node 6 was cut off (`TESTBED_ISOLATE=6:25:60`): from t = 25 s to t = 85 s it dropped every incoming block, inventory and tree message, while it kept mining and sending. Data: `experiments/fork_choice_test/`.

| Snapshot (s) | 0 | 32 | 60 | 92 | 120 | 153 | 211 (final) |
|---|---|---|---|---|---|---|---|
| Blocks on nodes 1–5 | 1 | 43 | 49 | 59 | 65 (node 4: 64) | 71 | 77 |
| Blocks on node 6 | 1 | 35 | 36 | 37 | 65 | 71 | 77 |
| Same block set on all six | yes | no | no | no | no | **yes** | **yes** |

- **During the cut** node 6 dropped 110 messages and built its own branch, which had 37 blocks when the cut ended, against 59 on the other nodes. Node 6 mines only in lane 3, so its branch grew slowly.
- **After the cut:**
  - Node 6's parent queue refused the other branch's blocks, which did not fit its own branch (26 rejected retries of pending blocks).
  - The other nodes' `TREE_STATUS` messages showed a better tree. Node 6 requested it once, validated it, and switched: "Switched to a better tree: 59 blocks (ours had 37), 5 transactions back in the mempool".
  - This happened between the snapshots at 92 s and 120 s, so within 35 s of the end of the cut.
- **One more switch:**
  - Node 4 mined a block on a template with an outdated target (the same transient as in Section 3.4). Its own check rejected that block, and for a moment node 4's tree differed from the others' by one block (64 against 65).
  - It requested the better tree and switched once.
- **No tree was rejected after a request**, and no pending block was added without its parent-queue entry.
- **At the end:**
  - All six nodes held identical trees of 77 blocks (76 mined plus genesis; lanes 26 / 31 / 19).
  - The slot rule and lane links hold, and all 76 blocks carry the bits the rule gives (2 retargets).
  - The live `/verify_tree` and `/verify_pqp` returned valid on all six nodes.
  - 57 transactions were confirmed, all in the lane of their inputs.

An earlier fork test, run before the fixes of Section 1, item 5, also ended with identical trees. Along the way:
- It needed 5 switches.
- Nodes repeatedly rejected trees they had requested ("does not fit the parent queue"). The trees really were inconsistent: the pending-block path had added blocks whose parent-queue entry had been rejected.

That run is not kept in this repository.

## 5. Limitations: what these runs can and cannot support

**They support these claims (functional validation):**
- The prototype runs end to end for 25 minutes with 6 nodes and 3 lanes, and no node stops or freezes.
- All nodes hold the same tree at the end and at every snapshot except while a block is in transit.
- The slot rule, lane links, PQP window and routing rule behave as specified.
- The difficulty rule retargets eight times, every node computes the same values, and the block rate settles near the target.
- The empty-slot rate and the age of parent references are close to the analytical predictions.
- When a node is cut off and builds its own branch, the longest-tree rule brings it back to the common tree.

**They do not support these claims:**
- **Throughput at scale:** one laptop with 6 miners sharing 10 cores, 6 wallets, and 25 minutes.
- **Behavior under real network delay:** on localhost there were no slot races and no late blocks in 25 minutes. The only fork came from the deliberate cut in the fork test.
- **Security:** the runs had no adversary. Receiving nodes also check each block's bits but do not re-check its hash against the target (the only such check is in the miner, `src/miner/routes.rs:213`).
- **The default parameters:** the difficulty ran with test values (15 s / 150 s) so that it would retarget within 25 minutes. The rules are the same as with the defaults.
- **Pure hardware effects:** the 25-minute run used a debug build, and the fanless Air may throttle under sustained load.

## 6. Files

In `experiments/local_6node_run/` (25-minute run) and `experiments/fork_choice_test/` (fork test):

| File | Contents |
|---|---|
| `run_local_testbed.py` | Starts the nodes, drives the workload, takes snapshots, dumps the final state (only in `local_6node_run/`) |
| `analyze_run.py` → `analysis.json` | All numbers in this file, including the offline check of every block's bits (script only in `local_6node_run/`) |
| `fill_rate_montecarlo.py` | Monte Carlo of the window rule, with and without a head start for the finder of the previous block (only in `local_6node_run/`) |
| `config_used.json` | The config values used for the run |
| `meta.json`, `driver.log` | Run parameters, wallets, code version, and the driver's log |
| `timeline.jsonl` | 30-second snapshots of every node |
| `tx_log.jsonl` | Every transaction submission and its result |
| `final/` | Per-node final dumps, including blocks, PQP and verification results; `verify_tree_recheck.json` holds the offline re-run of the fixed tree check (Section 3.2) |
| `logs/` | Full logs of the 6 nodes and the ports server (gzipped; read them with `gunzip -c`) |

The routing analysis on real Bitcoin transactions (Section VII-G of the paper) is in `experiments/bitcoin_lane_analysis/`, which has its own README. The previous 25-minute run (2026-09-22, older routing rule) is in the git history of `experiments/local_6node_run/` (commit `277ec15`).

To re-analyze a run (from the repository root):

```bash
python3 experiments/local_6node_run/analyze_run.py experiments/local_6node_run > experiments/local_6node_run/analysis.json
```

To repeat the 25-minute run, first set `MINING_RATE = 15_000` and `EXPECTED_TIME = 150_000` in `src/config.rs` (the repository keeps the defaults). Then run `cargo build`, and from the repository root run the command below, which writes to a new folder:

```bash
python3 experiments/local_6node_run/run_local_testbed.py "$(pwd)" experiments/run_repeat 1500
```

For the fork test, build with `cargo build --release` and run:

```bash
TESTBED_BUILD=release TESTBED_ISOLATE=6:25:60 python3 experiments/local_6node_run/run_local_testbed.py "$(pwd)" experiments/fork_repeat 180
```

A repeat run needs its own `config_used.json` for the difficulty check: copy the one from `experiments/local_6node_run/`.

To re-run the tree check offline on a saved node state (with the run's `MINING_RATE` and `EXPECTED_TIME` set in `src/config.rs`):

```bash
BLOCKS_DUMP=experiments/local_6node_run/final/blocks_node1.json PQP_DUMP=experiments/local_6node_run/final/pqp_node1.json cargo test verify_saved_tree -- --ignored --nocapture
```
