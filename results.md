# Results: local 6-node run of the TreeChainTheory PoW prototype

**What this is:** a real 25-minute run of the prototype. Six node processes and the ports server ran on **one MacBook Air (Apple M4, 10 cores, 16 GB, macOS 27.0)**, all on `localhost`. The Rust code mined and validated every block; nothing here is simulated.

**What this is not:** a distributed deployment. There was no emulated network delay and no adversary. Read it as **functional validation** of the prototype, not as evidence for throughput at scale (see Section 4).

- **Date:** 2026-09-22. Mining ran from 13:51:15 to 14:16:16 IST (1,500 s).
- **Code:** commit `71c3b18` plus two bug fixes in the working tree (Section 1). Debug build (`cargo build`, Rust 1.97.0), the same kind of build the `.sh` script uses.
- **Config:** `CHILDREN = 3`, genesis `BITS = 1e1fffff`. For this run `MINING_RATE = 15 s` and `EXPECTED_TIME = 150 s`, so the difficulty is retargeted every 30 blocks; the repository defaults are 100 s and 10,000 s (every 300 blocks). With the defaults a 25-minute run never reaches the first retarget, so the difficulty rule would not be exercised. All other rules are unchanged.
- **Raw data and scripts:** `experiments/local_6node_run/`.

## Summary

| Check | Result |
|---|---|
| Node processes alive at every 30 s snapshot | **306 / 306** (6 nodes × 51 snapshots) |
| Snapshots answered without an error | **306 / 306**; slowest `/get_blocks` reply 41 ms |
| Snapshots in which all six nodes held the same block set | **51 / 51** |
| Final trees | **identical on all six nodes**: 252 blocks (251 mined + genesis) |
| `/verify_tree` and `/verify_pqp` | valid on all six nodes |
| Difficulty retargets | **8**, and all 251 blocks carry exactly the bits the rule gives (recomputed offline) |
| "Bits mismatch" rejections, resyncs to genesis, panics | **0, 0, 0** |
| Routing | **314 / 314** confirmed transactions were in a block of their sender's lane |

---

## 1. Code changes before this run

Earlier runs exposed two bugs in the node software. Both are fixed in the working tree (not yet committed). The protocol rules themselves (slots, PQP window, lane links, routing, block counts) are unchanged.

1. **Lock ordering** (`src/miner/routes.rs`, `src/miner/p2p_server.rs`). `/create_txn` took the UTXO-set lock before the tree lock, while the block handler took them in the opposite order. Under a steady stream of transactions this could freeze a node. All handlers now take the locks in one order (wallet → tree → PQP → UTXO set → mempool) and release them before any network send.
2. **Deterministic difficulty** (`src/treechain/treechain.rs`, `src/miner/routes.rs`). The old retarget rule started from the bits of the highest-slot block in each node's *own* view. When two nodes' views differed for a moment, they expected different bits for the same block. In an earlier test run with this config, the nodes agreed on the first six retargets, disagreed from the seventh on, and their trees split. The new function `TreeChain::bits_for_upcount` gives every block of a 30-block period the same bits. It derives them only from the previous period's bits and the timestamps of blocks that come before the period's sibling groups. The miner and the verifier both use it. A unit test covers it (`bits_for_upcount_is_fixed_per_period_and_ignores_later_blocks`).

Unit tests: 17 of 18 pass. `test_get_prev_pqp_only_genesis` fails regardless of these changes, because it still expects the genesis commitment from before commit `6726e5f`.

## 2. Setup

| Node | ALIGN (mining lane) | HTTP / P2P port | Wallet's transaction lane* |
|---|---|---|---|
| 1 | 1 | 3001 / 5001 | 3 |
| 2 | 2 | 3002 / 5002 | 1 |
| 3 | 3 | 3003 / 5003 | 2 |
| 4 | 1 | 3004 / 5004 | 1 |
| 5 | 2 | 3005 / 5005 | 3 |
| 6 | 3 | 3006 / 5006 | 3 |

\* `(last hex digit of the wallet's public key % 3) + 1`, the prototype's routing rule. Keys are generated when a node starts; this time three wallets landed in lane 3, two in lane 1 and one in lane 2.

- **Nodes:** started as `starteverything_startmining.sh` does: the ports server, then six nodes 0.5 s apart, then `/start_mining` on each. `tmux` is not installed on this Mac, so the same commands ran as background processes. The frontends were skipped because they only display data. Every node had 5 peers before mining started.
- **Workload:** every 4 s, each wallet without an unconfirmed payment paid a random other wallet 0.1 to 1 coin with a 10,000-sat fee through `POST /create_txn`. These are real signed P2PKH transactions. Each wallet had at most one unconfirmed payment at a time, because the prototype's wallet always spends its oldest UTXOs and a second payment would double-spend the first.
- **Snapshots:** every 30 s the driver recorded each node's blocks, PQP size, mempool, peers, whether its process was alive, and how long it took to answer.
- **End of run:** after 1,500 s the driver called `/stop_mining` on every node and waited 20 s for in-flight blocks. It then took a final snapshot and saved each node's blocks, PQP, children map, wallet, UTXO set, mempool, peers, and the results of `/verify_tree` and `/verify_pqp`.

## 3. Results

### 3.1 No node stopped

- All six node processes ran from start to finish. At all 51 snapshots every process was alive and answered every request; the slowest `/get_blocks` reply took 41 ms.
- The node logs contain no panics, errors or timeouts. The frequent line "Data too short for message length: 0 bytes" is normal: the message parser prints it whenever its buffer is empty. All six nodes answered `/stop_mining` and the final dumps before the driver shut them down.
- Every node kept mining to the end. Each node's last block in the tree was mined at t = 1,482 / 1,486 / 1,434 / 1,489 / 1,477 / 1,463 s (nodes 1–6).

### 3.2 Consistency and validity

- **Snapshots:** all six nodes held the same block set at every one of the 51 snapshots, and their block counts never differed.
- **Final trees:** identical on all six nodes, with 252 blocks (251 mined plus genesis). `/verify_tree` returned "Tree is valid" and `/verify_pqp` returned "PQP is valid" on every node.
- **Independent checks on the final tree (all passed):**
  - The slot rule `queue_index = 3 * parent_queue_index + align` holds for all 251 blocks.
  - Every `prev_pqp_commitment` equals the `pqp_commitment` of the nearest earlier block in the same lane.
- **Races and resyncs:**
  - 1 same-slot race: node 5 found a block for a slot that node 2 (the other lane-2 miner) had filled a moment earlier. Node 5 dropped its block before broadcasting it (first seen wins).
  - 0 blocks rejected for pointing at a retired parent, 0 blocks queued for a missing parent, 0 resyncs to genesis.

### 3.3 Block production

| Metric | Value |
|---|---|
| Blocks mined (excluding genesis) | 251 (the last one at t = 1,489 s) |
| Overall rate | 10.1 blocks/min (target: 12/min, one block every 5 s) |
| Interval between blocks, any lane | mean 5.96 s, median 3.97 s |
| Blocks per lane (1 / 2 / 3) | 94 / 78 / 79 |
| Interval within a lane | mean 16.0 / 19.0 / 18.8 s, median 10.3 / 12.3 / 10.8 s (target 15 s) |
| Blocks in the tree per miner (nodes 1–6) | 50 / 44 / 36 / 44 / 34 / 43 |
| Mining restarts because a new block arrived | 194–211 per node |

Block rate in 5-minute windows:

| Window (s) | 0–300 | 300–600 | 600–900 | 900–1200 | 1200–1500 |
|---|---|---|---|---|---|
| Blocks | 31 | 42 | 68 | 62 | 47 |
| Seconds per block | 9.7 | 7.1 | 4.4 | 4.8 | 6.4 |

The first ten minutes ran at the genesis difficulty and the first two retargets. **From t = 600 s the tree averaged 5.1 s per block (177 blocks in 900 s), against the 5 s target.**

### 3.4 Difficulty adjustment

The target is one block per lane every 15 s, so one block every 5 s across the three lanes. Period *k* covers the blocks whose count (real blocks before the sibling group, plus the lane) is between 30*k* and 30*k* + 29. Its bits come from period *k*−1's bits, scaled by the time between the blocks with counts 30(*k*−1)−3 and 30*k*−3. That time is normalized to 30 block intervals and compared with 150 s, and the factor is clamped to [¼, 4].

| Period | Counts | Bits | Difficulty vs genesis | Retarget window (counts, duration) | Target factor | Blocks | Mean interval |
|---|---|---|---|---|---|---|---|
| 0 | 0–29 | `1e1fffff` | 1.00 | (genesis bits) | – | 28 | 10.6 s |
| 1 | 30–59 | `1e429d7f` | 0.48 | 2–27, 260.2 s (312.3 s scaled to 30) | 2.08 | 30 | 7.9 s |
| 2 | 60–89 | `1e53a95f` | 0.38 | 27–57, 188.4 s | 1.26 | 30 | 7.5 s |
| 3 | 90–119 | `1e855f25` | 0.24 | 57–87, 239.1 s | 1.59 | 30 | 3.5 s |
| 4 | 120–149 | `1e584256` | 0.36 | 87–117, 99.3 s | 0.66 | 30 | 5.1 s |
| 5 | 150–179 | `1e4b55ec` | 0.43 | 117–147, 128.0 s | 0.85 | 28 | 4.9 s |
| 6 | 180–209 | `1e4f27b7` | 0.40 | 147–177, 157.6 s | 1.05 | 32 | 6.1 s |
| 7 | 210–239 | `1e599f1e` | 0.36 | 177–207, 169.8 s | 1.13 | 30 | 6.2 s |
| 8 | 240–269 | `1e750d16` | 0.27 | 207–237, 195.9 s | 1.31 | 13 (run ended) | 3.0 s |

"Difficulty vs genesis" is the genesis target divided by the period's target (below 1 means easier). The target factor is new target ÷ old target. Mean intervals are approximate: a period is defined by counts, not by time, so neighboring periods overlap by a few seconds.

- **Every node agreed on every retarget.** All 251 blocks carry exactly the bits that an offline re-implementation of `bits_for_upcount` computes from the final tree. No node logged a "Bits mismatch". All six nodes verified the same values for the first sibling group of each period (24 blocks).
- **The adjustment worked as intended.** The genesis difficulty was too hard for six debug-build miners on this laptop (10.6 s per block), so the first two retargets made mining easier. Period 3 overshot (3.5 s per block) because each window measures the previous period, as in Bitcoin, and the next retarget corrected it. From period 4 on, the mean interval stayed between 4.9 and 6.2 s.
- **Two brief template changes, no disagreement:**
  - What happened: at the start of periods 3 and 8, the two lane-3 miners (nodes 3 and 6) built a template while one block of the previous sibling group had not reached them yet. When it arrived, their block's count and the last block of the retarget window both changed, and both miners rebuilt the template with the agreed bits before finding a block. No block ever carried the other value.
  - When it can happen: only for the first sibling group of a period.
  - How to avoid it: ending the window one sibling group earlier (count 30*k*−6) would remove it.
- **Noisy window measurement:** the window measures the time between two blocks in slot order. Siblings arrive in any order (56 of the 250 consecutive pairs in slot order have decreasing timestamps), so one window can be off by the gap between two siblings. Using the median timestamp of a sibling group would smooth this.

### 3.5 Tree shape and empty slots

| Metric | Value |
|---|---|
| Closed sibling groups (parents already retired) | 140 |
| Slots in those groups | 420 |
| Filled / empty | 249 / 171, a **fill rate of 59.3%** |
| Children per closed parent | 1 child: 60 parents, 2 children: 51, 3 children: 29 (mean 1.78) |
| Tree depth | 8 levels |
| Highest slot index | **3,785**, so each node stores about 3,530 placeholder records for 251 real blocks |

A Monte Carlo of the window rule alone (`fill_rate_montecarlo.py`: equal hash power per lane, zero delay) predicts a 63.0% fill rate for N = 3. With this run's lane split (94 / 78 / 79) the prediction is 62.7%.
- **Compared with the prediction:** 59.3% is 1.7 standard deviations below it, and the gap is mostly in single-child parents: 60 of 140 (43%) against 33% in the model. The model produces that many single-child parents in a 140-group sample only about 1% of the time, so chance alone is an unlikely explanation.
- **Cause: the finder's head start.**
  - When a node receives a block, it stops mining, validates the block, sleeps 100 ms and rebuilds its template. The node that found the block starts its next template at once.
  - In this run the next block came from the **same node 23.6%** of the time, against 16.7% for six equal miners. The other miner of the same lane had no advantage (16.4%).
  - A parent ends with one child exactly when one lane finds two blocks in a row, so the head start adds single-child parents.
  - `fill_rate_montecarlo.simulate_with_finder_advantage` adds this head start to the model, with the finder's rate ×1.55, calibrated to the measured 23.6%. It then predicts a **59.9%** fill rate and 39% single-child parents, close to the measured 59.3% and 43%.
  - Numbers: `analysis.json` → `next_block`.
- **What to take from it:** the fill rate depends on the mining loop as well as on the window rule. A miner should restart only when an incoming block takes its slot or retires its parent, not on every block.
- **Earlier run:** the first run (default config, blocks every ~11 s, so restarts mattered less) measured 62.4%, and there the same node found the next block 15.9% of the time.

The conclusions for the paper stay the same:
- Empty slots come from the window rule itself, not from the network.
- They do not waste mining work, because a miner whose parent retires simply moves on to the next parent.
- Slot indices grow much faster than the block count: slot 3,785 after 251 blocks. The prototype stores a placeholder for every empty slot and uses a `u32` index. The paper recommends sparse slot storage and 64-bit indices.

### 3.6 Age of parent references (checks the paper's Observation 1)

- Measured in counts (position in slot order), the median of parent count ÷ block count is **0.55**, and **0.56** in the second half. The prediction 1/(fill rate × N) = 1/(0.593 × 3) is **0.56**.
- Measured in time, the median of parent time ÷ block time is 0.61 (0.62 in the second half). It is higher because the first blocks came slowly at the genesis difficulty, so the early counts cover more time.
- So a recent block points to a parent created about halfway back in the tree's history. At first only the block's own lane links protect it, as the paper argues.

### 3.7 Transactions

| Metric | Value |
|---|---|
| Submission attempts | 485 |
| Accepted into a mempool | 318 |
| Confirmed in blocks | 314; the other 4 were submitted in the last 33 s and sat in every node's mempool at the end (the same 4 on all nodes) |
| Confirmed per lane (1 / 2 / 3) | 119 / 51 / 144 (lanes with 2, 1 and 3 sending wallets) |
| Routing check | **314 / 314** confirmed transactions were in a block of their sender's lane |
| Confirmation latency (submission → block) | median 13.5 s, mean 18.3 s, 90th percentile 46.2 s |
| Confirmed throughput | 12.7 tx/min (limited by the workload; see below) |

Rejected submissions, all at the start of the run:

| Count | Reason | When |
|---|---|---|
| 80 | "insufficient funds": the wallet had no coins yet | all before t = 121 s |
| 87 | "coinbase UTXO not mature (requires 10 confirmations)" | all before t = 242 s |

After t = 242 s every submission was accepted. Blocks carried 1.25 user transactions on average (at most 3), against a capacity of about 11 (10,240 weight units). The throughput above therefore reflects the six-wallet workload, not what the tree can carry.

## 4. Limitations: what this run can and cannot support

**It supports these claims (functional validation):**
- The prototype runs end to end for 25 minutes with 6 nodes and 3 lanes, and no node stops or freezes.
- All nodes hold the same tree at every snapshot and at the end.
- The slot rule, lane links, PQP window and routing rule behave as specified.
- The difficulty rule retargets eight times, every node computes the same values, and the block rate settles near the target.
- The empty-slot rate and the age of parent references are close to the analytical predictions.

**It does not support these claims:**
- **Throughput at scale:** one laptop with 6 miners sharing 10 cores, 6 wallets, and 25 minutes.
- **Behavior under real network delay:** on localhost there was one slot race and no late blocks.
- **Security:** the run had no adversary. Receiving nodes also check each block's bits but do not re-check its hash against the target (the only such check is in the miner, `src/miner/routes.rs:208`).
- **The default parameters:** the difficulty ran with test values (15 s / 150 s) so that it would retarget within 25 minutes. The rules are the same as with the defaults.
- **Pure hardware effects:** it used a debug build, and the fanless Air may throttle under sustained load.

## 5. Files (in `experiments/local_6node_run/`)

| File | Contents |
|---|---|
| `run_local_testbed.py` | Starts the nodes, drives the workload, takes snapshots, dumps the final state |
| `analyze_run.py` → `analysis.json` | All numbers in this file, including the offline check of every block's bits |
| `fill_rate_montecarlo.py` | Monte Carlo of the window rule, with and without the finder's head start |
| `config_used.json` | The config values used for this run |
| `meta.json`, `driver.log` | Run parameters, wallets, code version, and the driver's log |
| `timeline.jsonl` | 30-second snapshots of every node |
| `tx_log.jsonl` | Every transaction submission and its result |
| `final/` | Per-node final dumps, including blocks, PQP and verification results |
| `logs/` | Full logs of the 6 nodes and the ports server (gzipped; read them with `gunzip -c`) |

The routing analysis on real Bitcoin transactions (Section VII-G of the paper) is in `experiments/bitcoin_lane_analysis/`, which has its own README.

To re-analyze this run (from the repository root):

```bash
python3 experiments/local_6node_run/analyze_run.py experiments/local_6node_run
```

To repeat it, first set `MINING_RATE = 15_000` and `EXPECTED_TIME = 150_000` in `src/config.rs` (the repository keeps the defaults). Then run `cargo build`, and from the repository root run the command below, which writes to a new folder:

```bash
python3 experiments/local_6node_run/run_local_testbed.py "$(pwd)" experiments/run_repeat 1500
```

The repeat run needs its own `config_used.json` for the difficulty check: copy the one from `experiments/local_6node_run/`.
