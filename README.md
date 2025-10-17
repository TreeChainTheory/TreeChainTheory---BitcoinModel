# 🌳 TreeChainTheory – Bitcoin Model  
> **Reinventing BlockChain with Tree-Structured Consensus**

TreeChainTheory – Bitcoin Model extends the **TreeChainTheory** concept into a working prototype based on Bitcoin’s architecture.  
Instead of a linear chain, blocks are organized in a **tree**, allowing **parallel mining**, **multi-leader operation**, and **scalable block production** — all while preserving **Bitcoin’s UTXO**, **PoW**, and **ECDSA** foundations.

---

## 🧭 Overview

- 🔗 **Model Type:** Proof-of-Work (Bitcoin-like)
- ⚙️ **Implementation:** Rust (Actix, Tokio, Serde)
- 🌿 **Structure:** Tree-based blockchain with dynamic **Parent Queue Pool (PQP)**
- 🧱 **Consensus:** Race-based PoW with PQP alignment rules  
  *(see [consensus.md](./consensus.md))*
- 🌐 **Networking:** P2P sync with inventory broadcast and IBD
- 💰 **Economics:** Bitcoin-like subsidy, halving, and difficulty adjustments
- 📂 **Execution Guide:** [HowToRun.md](./HowToRun.md) , [Direct Api Access](./ApiAccess.md)

---
## 🌳 Discription
Each **block** will have a specified number of **Children**, denoted as **CHILDREN** num. Every child is assigned an alignment value **(align:N)** from 1 to CHILDREN, referred to as **1-aligned miner, 2-aligned miner**, and so on.
These different **aligned miners** mine their respective aligned blocks under one **condition:** they must complete mining their aligned block for the current parent **before** the next parent receives any children, this is called **Parent-Child Completion Constraint**.

> **CHILDREN** — a configurable constant defined in `src/config.rs`. (can be modified as you wish) 
> It specifies the **maximum number of child blocks** that any parent block can produce.


**Parent-Child Completion Constraint** – if N aligned miner finishes mining the N aligned block for the current parent , then that miner can start mining the N aligned block for the next parent And all aligned miners must finish their block work for the current parent before the next parent gets any children.  

---

## 🌲 Tree Structure & Parent Queue Pool (PQP)

TreeChainTheory replaces the linear block list with a **branching tree** — each parent may produce multiple children (default: **3**).  
This introduces **parallelism**, **fairness**, and **decentralization** into the mining process.

### 🔍 **Core TreeChain Principles**
- **🌳 Branching:**  
  Each parent block produces up to `CHILDREN` (default `3`) children.  
  The system supports **N-ary trees** via config changes.
- **⚡ Parallel Mining:**  
  Multiple leaders mine simultaneously — one per align (child position).
- **🔄 Dynamic Scheduling (PQP):**  
  The **Parent Queue Pool** manages all eligible parents, using a **breadth-first queue** to maintain balanced tree growth.
- **🧩 Deterministic Indexing:**  
  - `queue_index`: Assigned via BFS — genesis = 0, children numbered left-to-right per level.  
  - `align`: Integer 1…CHILDREN, defines child position.  
- **🔒 Commitment & Integrity:**  
  Each PQP entry includes:  
  `queue_index + align + block_hash + parent_hash + miner_address + prev_pqp_commitment + signature`  
  → hashed into `pqp_commitment` for verifiable linkage.  
- **🧾 Miner Verification:**  
  `signature` (ECDSA) validates the miner’s authority; all fields are hex-encoded for transparency.  
- **📜 Transaction Balancing:**  
  Miner selects txns where `(last_digit % CHILDREN) + 1 == align` to ensure even load.  
- The **PoW model** does not guarantee exactly `CHILDREN` blocks per parent as we are following the **Parent-Child Completion Constraint** — but future **PoS/PoH** models will.

> 🔗 *For full consensus and PQP validation process, see [consensus.md](./consensus.md).*

---

## ⛏ Mining & Consensus – Bitcoin Model

- **PoW:** Standard nonce iteration until `hash < target`
- **Mining Rate:** Every `MINING_RATE` ms (default: `100ms`)
- **Difficulty:** Retargeted every `300` blocks, clamped between ¼× and 4×
- **Halving:** Subsidy halves every `HALVING_INTERVAL` (default `100` blocks)
- **Block Template:**
  - Coinbase transaction (subsidy + fees)
  - Merkle root of transactions
  - PQP entry signed by miner
- **Queue Index:**  
  `queue_index = max(parent QI) + align offset`
- **Parent-Child Completion Constraint**
  A miner must mine its aligned block **before the next parent** gets any children.

---

## 💰 UTXO & Transaction Layer

- **Model:** Bitcoin-style UTXO set with `queue_index` maturity tracking  
- **Supported Scripts:**
  - `P2PKH`
  - `P2SH` multisig (m-of-n)
- **Validation Includes:**
  - Signature and redeem script checks  
  - Fee validation (min sat/vB)
- **Transaction Creation:**
  - Input selection  
  - `SIGHASH_ALL`  
  - `ECDSA` signing (`k256`)
- **Coinbase Maturity:**  
  Spendable after 10 confirmations

---

## 🌐 P2P Networking

- **Framework:** Tokio TCP  
- **Core Messages:**  
  - `MINED_BLOCK`  
  - `INV_MESSAGE`  
  - `TRANSACTION`  
- **Synchronization:** IBD (`getblocks` / inventories)  
- **Topology:**  
  - `parent_hash` + `children_map` define tree topology  
  - Depth tracked as hierarchical “positions” (e.g., `0.1.2`)
- **Verification:**  
  - Parent existence  
  - PQP commitment  
  - Transaction validity  
- **Reorgs:**  
  - Safe invalidation and rollback of outdated transactions
 
---

## 🧠 Design Insights

- **Parallel Mining → Higher Throughput**
- **Queue-based Scheduling → Fairness**
- **Tree-based Linking → Reduced Centralization**
- **UTXO & PQP Integration → Determinism + Auditability**

> ⚙️ **Configurable Scaling:**  
> Modify `CHILDREN` in `TreeChainTheorey---BitcoinModel/src/config.rs` to experiment with larger tree branching.

---

## 📈 Summary

TreeChainTheory – Bitcoin Model demonstrates that a **tree-structured blockchain** can achieve:
- 🚀 **Higher throughput** vs. linear chains  
- 🔒 **Enhanced miner fairness** via PQP  
- 🌿 **Scalable, parallel consensus** without sharding  
- 💡 **Seamless integration with Bitcoin’s UTXO logic**

> 🧩 **Next Step:** Explore how Proof-of-Stake (PoS) or Proof-of-History (PoH) variants can guarantee consistent CHILDREN production and further scalability. POS/POH models will be available soon

---

## 📚 Documentation Links

- ⚙️ [How to Run the Node →](./HowToRun.md)
- 🧾 [Consensus & PQP Theory →](./consensus.md)

---
## ⚠️ Notes

- All hashes and addresses are **hex-encoded for human readability** in this prototype.  
- The **PoW model** does not guarantee exactly `CHILDREN` blocks per parent — but future **PoS/PoH** models will.

---

## 🧑‍💻 Built With

- 🦀 **Rust** — core logic & mining engine  
- ⚡ **Tokio** — async P2P networking  
- 🧠 **Actix** — API & simulation interface  
- 🔐 **k256** — ECDSA key handling  
- 🧮 **num-bigint** — PoW target math  

---

> “What if a blockchain wasn’t a chain?”  
> — *TreeChainTheory, 2025*
