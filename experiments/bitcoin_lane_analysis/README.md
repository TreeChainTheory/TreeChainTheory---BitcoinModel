# Lane routing on real Bitcoin transactions

This analysis checks how DetTreeChain's routing rule would spread **real Bitcoin owners** over lanes. It uses **200 randomly chosen real blocks** from the last year of the Bitcoin chain, and its numbers are the ones in Section VII-G and Fig. 8 of the paper.

**The rule (the prototype's rule since 2026-10-04):**

```
lane(output) = (first 64 bits of SHA-256(locking script of the output) mod N) + 1
```

A transaction may only spend outputs of one lane, and only blocks of that lane may include it. The locking script (`scriptPubKey`) says who may spend an output, so it identifies the owner, and every node knows an output's lane as soon as the output is created. It works the same for every kind of lock (single key, multisig, SegWit, timelocks). Wallets keep all their coins in one lane by using only addresses whose script maps to that lane, which takes about N key generations per address.

The prototype's earlier rule (last hex digit of the sender's public key) is evaluated alongside for comparison.

## Data

| | |
|---|---|
| Blocks | **200**, drawn uniformly at random (seed `20260922`) from the 52,560 blocks ending at height **968,126** (the chain tip on 2026-09-22) |
| Heights sampled | 916,071 – 967,292 |
| Dates | 2025-09-23 to 2026-09-16 (UTC) |
| Size | 300.6 MB of raw blocks in `raw_blocks/` |
| Source | public Esplora APIs (`blockstream.info/api`, `mempool.space/api`) |
| Integrity | a block is kept only if its header hashes to the block hash **and** its transactions hash to the header's Merkle root |
| Transactions | **762,842** non-coinbase transactions, with 1,426,014 inputs |

`blocks_index.csv` lists every block (height, hash, timestamp, size), so the raw blocks can always be downloaded again.

## Method

A spending transaction does not contain the locking script it spends, but for most input types it reveals enough to rebuild it exactly (`btc_parse.locking_script`):

| Input type | What the spend reveals | Rebuilt locking script |
|---|---|---|
| P2PKH | public key | `76a914 <HASH160(key)> 88ac` |
| P2WPKH | public key (witness) | `0014 <HASH160(key)>` |
| P2SH-wrapped P2WPKH | public key, redeem script `0014…` | `a914 <HASH160(redeem)> 87` |
| P2SH multisig | redeem script | `a914 <HASH160(redeem)> 87` |
| P2SH-wrapped P2WSH | redeem script `0020…` | `a914 <HASH160(redeem)> 87` |
| P2WSH | witness script | `0020 <SHA256(witness script)>` |

The rebuilt scripts were checked against the real spent outputs (fetched from the public API) for two inputs of each of the six types: **12 of 12 matched exactly**.

Taproot spends (key path and script path), bare public keys and some non-standard scripts do not reveal the locking script, so those transactions are left out. A DetTreeChain node has no such limitation: it reads the script from its UTXO set.

| Coverage | |
|---|---|
| Transactions whose first input reveals its locking script | **635,952 (83.4%)** |
| Inputs that reveal their locking script | 1,203,885 of 1,426,014 (84.4%) |
| Owners (distinct locking scripts of first inputs) | **253,292** |

## Results

### Lane shares for N = 3 (the prototype's configuration)

| | Lane 1 | Lane 2 | Lane 3 |
|---|---|---|---|
| Owners (distinct locking scripts) | 33.4% | 33.2% | 33.4% |
| Transactions | 33.3% | 38.4% | 28.3% |
| One block, 10th–90th percentile of the transaction share | 23.3–40.7% | 27.0–49.7% | 19.3–37.2% |

### All N from 2 to 8

Largest deviation of any lane from an even split (percentage points), and how many transactions already fit the rule:

| N | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|
| Script hash, owners | 0.01 | 0.18 | 0.10 | 0.05 | 0.13 | 0.13 | 0.11 |
| Script hash, transactions | 0.83 | 5.08 | 6.75 | 4.71 | 6.80 | 3.62 | 7.47 |
| Earlier rule (last key digit), keys | 0.10 | 4.26 | 0.12 | 4.98 | 4.18 | 4.46 | 0.14 |
| Multi-input transactions with all inputs in one lane | 60.5% | 54.5% | 51.1% | 49.7% | 48.7% | 48.0% | 47.2% |
| … expected if wallets ignore lanes | 60.7% | 54.2% | 51.3% | 49.7% | 48.6% | 47.9% | 47.3% |
| All transactions valid unchanged | 95.3% | 94.6% | 94.2% | 94.0% | 93.9% | 93.8% | 93.7% |

"Valid unchanged" counts the transactions whose inputs all reveal their scripts and already spend outputs of a single lane, as today's wallets create them.

### What it means

1. **Owners spread evenly for every N.** Hashing the locking script keeps every lane within 0.2 percentage points of an even split. The earlier last-digit rule is even only for N = 2, 4 and 8, and 4–5 points off otherwise. At N = 3, for example, lane 1 gets the digits 0, 3, 6, 9, c and f, which is 37.6% of the keys.
2. **Load does not spread evenly, whatever the rule.** The busiest 1% of owners appear in **58%** of the transactions, and one locking script appears in **22,343**, while 94% of owners appear only once. Routing keeps every transaction of an owner in one lane, which conflict freedom (Theorem 1) requires, so busy owners make busy lanes: up to 7.5 points off by transactions.
3. **Most transactions already fit the rule.** 88% of transactions spend a single output. Among multi-input transactions, 44% spend outputs of one locking script, and the share with all inputs in one lane matches what random assignment would give, as expected, since today's wallets don't know about lanes. Overall, **about 94–95%** of today's transactions would be valid as they are. The rest would be split, or created by lane-aware wallets that keep their addresses in one lane.

## Files

| File | Contents |
|---|---|
| `fetch_blocks.py` | Picks the 200 heights (fixed seed), downloads and verifies the blocks, writes `blocks_index.csv` |
| `btc_parse.py` | Raw-block parser (legacy + SegWit) with txid and Merkle-root checks; input classification; rebuilding of locking scripts |
| `analyze_lanes.py` | The analysis above → `results.json` |
| `results.json` | All numbers: lane shares for N = 2..8 by transactions and owners, per-block spread, concentration, multi-input statistics, earlier rule |
| `blocks_index.csv` | The 200 blocks: height, hash, timestamp, size |
| `fetch.log` | Download log (retries included) |
| `raw_blocks/` | The 200 raw blocks, **300 MB, not tracked by git** (see `.gitignore`) |

## Re-running

From this folder:

```bash
python3 fetch_blocks.py .
```

```bash
python3 analyze_lanes.py .
```

`fetch_blocks.py` skips blocks that are already on disk and complete. The analysis takes about 15 seconds.

## Deleting the raw blocks

The raw blocks are only needed to re-run the analysis; `results.json` and `blocks_index.csv` keep everything the paper uses. To free the 300 MB:

```bash
rm -rf raw_blocks
```

`fetch_blocks.py` downloads the same 200 blocks again at any time.
