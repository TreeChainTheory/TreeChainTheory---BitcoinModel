# Lane routing on real Bitcoin transactions

This analysis checks how DetTreeChain's routing rule would spread **real Bitcoin owners** over lanes. The rule puts a transaction into lane `(f(owner) mod N) + 1`, where the owner is the public key that authorizes the transaction's first input. It uses **200 randomly chosen real blocks** from the last year of the Bitcoin chain. Its numbers are the ones in Section VII-G and Fig. 8 of the paper.

## Data

| | |
|---|---|
| Blocks | **200**, drawn uniformly at random (seed `20260922`) from the 52,560 blocks ending at height **968,126** (the chain tip on 2026-09-22) |
| Heights sampled | 916,071 – 967,292 |
| Dates | 2025-09-23 to 2026-09-16 (UTC) |
| Size | 300.6 MB of raw blocks in `raw_blocks/` |
| Source | public Esplora APIs (`blockstream.info/api`, `mempool.space/api`) |
| Integrity | every block was kept only if its header hashes to the block hash **and** its transactions hash to the header's Merkle root |
| Transactions | **762,842** non-coinbase transactions |

`blocks_index.csv` lists every block (height, hash, timestamp, size). The raw blocks can always be downloaded again from it.

### What the first inputs look like

| First input of the transaction | Transactions | Share |
|---|---|---|
| P2WPKH (native SegWit, key in the witness) | 573,439 | 75.2% |
| Other SegWit (script paths, P2WSH non-multisig, …) | 73,412 | 9.6% |
| P2TR key path (key only in the spent output) | 38,269 | 5.0% |
| **P2PKH (legacy, key at the end of the unlocking script)** | **28,921** | **3.8%** |
| Other legacy scripts | 19,658 | 2.6% |
| P2SH-wrapped P2WPKH | 16,807 | 2.2% |
| P2WSH multisig | 11,503 | 1.5% |
| P2SH multisig | 833 | 0.1% |

## Method

- **Owner of a transaction:** the public key that authorizes its first input, as in the prototype, which routes by `vin[0]`.
- **Two groups of transactions:**
  - `p2pkh`: the first input is a legacy P2PKH spend. This is exactly the case the prototype implements, because the key ends the unlocking script. It covers 28,921 transactions and 23,031 distinct keys.
  - `single_key`: the first input is P2PKH, P2WPKH or P2SH-P2WPKH, with the key in the script or the witness. It covers 619,167 transactions (81.2% of all) and 238,419 distinct keys.
- **Two owner functions `f`:**
  - `last_hex_digit`: the last hexadecimal digit of the key. This is the prototype's rule.
  - `sha256`: the first 64 bits of SHA-256 of the key. This is the hash-based rule the paper recommends.
- **Measures:** for N = 2..8, the share of each lane, counted by **transactions** and by **distinct keys**, and the largest deviation from an even split (100/N %), in percentage points.

## Results

### N = 4 (the setting of the original draft)

| Group | Rule | Lane 1 | Lane 2 | Lane 3 | Lane 4 |
|---|---|---|---|---|---|
| P2PKH, by transactions | last hex digit | 23.9% | 24.3% | 26.7% | 25.1% |
| P2PKH, by keys | last hex digit | 25.4% | 24.8% | 25.1% | 24.7% |
| Single-key, by transactions | last hex digit | 17.4% | 22.9% | 32.6% | 27.1% |
| Single-key, by keys | last hex digit | 24.9% | 25.1% | 25.0% | 25.0% |

Between blocks, a lane's share of the ~145 P2PKH transactions in a block ranges from about 18% to 33% (10th to 90th percentile, 183 blocks with at least 20 of them).

### N = 3 (the prototype's configuration)

| Group | Rule | Lane 1 | Lane 2 | Lane 3 |
|---|---|---|---|---|
| Single-key, by keys | last hex digit | **37.6%** | 31.1% | 31.3% |
| Single-key, by keys | SHA-256 | 33.3% | 33.4% | 33.2% |
| P2PKH, by keys | last hex digit | **37.8%** | 30.9% | 31.2% |
| P2PKH, by keys | SHA-256 | 33.3% | 33.4% | 33.3% |

Lane 1 receives the six digits 0, 3, 6, 9, c and f (6/16 = 37.5%), so the last-digit rule is biased whenever N does not divide 16.

### Largest deviation from an even split, single-key group (percentage points)

| N | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|
| Last hex digit, keys | 0.10 | 4.26 | 0.12 | 4.98 | 4.18 | 4.46 | 0.14 |
| Last hex digit, transactions | 0.01 | 5.77 | 7.57 | 5.54 | 6.70 | 4.38 | 4.48 |
| SHA-256, keys | 0.22 | 0.11 | 0.14 | 0.12 | 0.13 | 0.04 | 0.10 |
| SHA-256, transactions | 3.21 | 1.20 | 2.40 | 7.68 | 5.61 | 4.30 | 4.67 |

### What it means

1. **Owners spread evenly with a hash-based rule.** By distinct keys, SHA-256 stays within 0.25 points of an even split for every N. The last-digit rule is even only for N = 2, 4 and 8, and is 4–5 points off otherwise, exactly as the digit counts predict.
2. **Load does not spread evenly, whatever the rule.** A few owners sign most transactions: in the single-key group the busiest 1% of keys sign **59%** of the transactions, and one key signs **22,343** of them. In the P2PKH group, the busiest 1% sign 16.9% and the top key 390. Routing keeps every transaction of an owner in one lane, which conflict freedom (Theorem 1) requires, so busy owners create busier lanes. By transactions, deviations reach 7.7 points.
3. For the prototype's own case (P2PKH, N = 4) the split is close to even: 23.9 / 24.3 / 26.7 / 25.1%.

## Files

| File | Contents |
|---|---|
| `fetch_blocks.py` | Picks the 200 heights (fixed seed), downloads and verifies the blocks, writes `blocks_index.csv` |
| `btc_parse.py` | Minimal raw-block parser (legacy + SegWit) with txid and Merkle-root verification; input classification |
| `analyze_lanes.py` | The analysis above → `results.json` |
| `results.json` | All numbers (shares per lane for N = 2..8, both rules, both groups, per-block spread, key reuse) |
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

`fetch_blocks.py` skips blocks that are already on disk and complete. The analysis takes a few seconds.

## Deleting the raw blocks

The raw blocks are only needed to re-run the analysis. `results.json` and `blocks_index.csv` keep everything the paper uses. To free the 300 MB:

```bash
rm -rf raw_blocks
```

They can be downloaded again at any time with `fetch_blocks.py`, which picks the same 200 blocks.
