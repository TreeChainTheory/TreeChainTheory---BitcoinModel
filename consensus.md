# 🌳 TreeChainTheorey Consensus — Bitcoin Model

## Consensus Overview

- **Consensus Type:** Proof-of-Work (PoW) — inspired by Bitcoin, adapted for a tree-structured blockchain.
- **Core Principle:** A parent block can simultaneously produce multiple children (Maximum **CHILDREN** num of blocks), enabling **parallel mining** and **increased throughput**.
  > **CHILDREN** — a configurable constant defined in `src/config.rs`.  
  > It specifies the **maximum number of child blocks** that any parent block can produce.

- **Queue Index (QI):**  
  - Assigned via **breadth-first traversal** — genesis = 0, children numbered left to right per level.
  - Queue Index is typically index of the block.
  - Ensures deterministic, ordered scheduling within the **Parent Queue Pool (PQP)**.
- **Parent Queue Pool (PQP):**
  - A decentralized double-ended queue managing eligible parent blocks for child creation.
  - Ensures **fair rotation** of parents and prevents bottlenecks.
  - Each PQP entry is **embedded in the block** for verifiable state consistency.
- **Race Rule:**  
  - A miner with a specific alignment (`align = X`) must mine its block **before the next parent** gets any children.
  - This maintains fair leader selection and synchronized growth of tree branches.
- **Block Linking:**
  - Each block stores `parent_hash`, `queue_index` and `prev_pqp_commitment` to establish its position in the tree.
  - Child blocks reference their parents, forming verifiable hierarchical relationships.
- **Mining Frequency:**  
  - Blocks are mined every `MINING_RATE` ms (default: 100_000 ms).
  - Difficulty (`BITS`) adjusts every **300 blocks** to maintain steady timing.
- **Reward & Halving:**  
  - Block subsidy halves every `HALVING_INTERVAL` (1000 blocks).
  - Total reward = **subsidy + transaction fees**.
- **PQP Commitments:**
  - Each entry includes a `prev_pqp_commitment` and a `pqp_commitment` — both SHA-256 hashes ensuring cryptographic linkage and verifiable updates.
  - `signature` (ECDSA) proves miner authenticity.
- **Transaction Assignment P2PKH:**
  - Transactions are distributed across alignments using:  
    `(last_digit(tx.vin[0].script_sig) % CHILDREN) + 1 == align`
  - Ensures **balanced transaction load** among child branches.
- **Other Types of Transactions Exception:**  
  - For other types of transactions, `parent_hash` (or `vin.txid` in this Bitcoin model) is used for the alignment condition.
- **Rollback Mechanism:**  
  - Malicious or invalid subtrees can be pruned and valid parents re-queued.
  - *Currently disabled in the Bitcoin model; reserved for future versions.*

