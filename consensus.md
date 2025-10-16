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
  - A decentralized queue managing eligible parent blocks for child creation.
  - Ensures **fair rotation** of parents and prevents bottlenecks.
  - Each PQP entry is **embedded in the block** for verifiable state consistency.
- **Parent-Child Completion Constraint:**  
  - A miner with a specific alignment (`align = X`) must mine its block **before the next parent** gets any children.
  - This maintains fair and synchronized growth of tree branches.
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

---
# 📦 Block 

Each block in **TreeChainTheorey** represents a node in the blockchain tree.  
Unlike Bitcoin’s strictly linear chain, TreeChainTheorey allows **multiple child blocks per parent**, forming a verifiable tree of blocks.

## 🧱 Overview

- A **Block** ties together:
  - Its **parent’s identity** (`parent_hash`)
  - Its bfs index (`queue_index`)
  - A **commitment to the previous block of the same alignment** (`prev_pqp_commitment`), ensuring continuity and verifiable linkage between sibling branches.
  - A **commitment to the current block** (`pqp_commitment`)
  - A **list of transactions**,
  - And a **proof-of-work** result (nonce + bits).

- Each block references one **PQP entry**, ensuring that creation is tied to a miner who is currently **eligible** to produce that block.

## 🪧 Fields Explanation

#### 🔗 `hash`
- SHA-256 hash of the entire block’s contents.`(except the pqp_commitment)`
- Serves as the **unique identifier** for the block.
- Used to validate that the block’s contents have not been altered.

### 🪞 `pqp_commitment`
- SHA-256 hash of selected fields (`queue_index`,`align`,`hash`,`parent_hash`,`miner_address`,`prev_pqp_commitment`,`signature`) that connect the block to the **Parent Queue Pool (PQP)**.
- Ensures the PQP entry and its state are **verifiably linked** to this block.
- Used by peers to confirm that the miner was authorized to create this block.
- **pqp_commitment** is computed after the **hash** is computed.

### 🌲 `level`
- Indicates the **depth** of the block in the tree (genesis = 0).
- Helps nodes determine the block’s position during traversal and validation.

### 🧭 `position`
- A **string representation** of the block’s exact placement in the tree (e.g., `"0.1.2"`).
- Used for visualization and queue index mapping.

### ⚙️ `version`
- Represents protocol version.  
- Useful for **future upgrades** (e.g., BIP upgrades).

### 🧩 `parent_hash`
- The hash of this block’s **parent block**.
- Establishes the tree linkage — every non-genesis block must reference one valid parent and a valid prev_pqp_commitment.

### 🌿 `merkle_root`
- Root hash of all transactions within this block.  
- Constructed by recursively hashing pairs of transactions until one hash remains.  
- Ensures the **integrity of all transactions** — if even one changes, the root changes.

### ⏰ `timestamp`
- Time at which the block was mined (as `u128` UNIX epoch ms).  
- Used for difficulty adjustment and chronological ordering.

### 🧮 `bits`
- Compact representation of the **difficulty target**.  
- Used by miners to determine if their block hash meets the current target (`hash < target`).

### 🔁 `nonce`
- A number that miners repeatedly modify to find a hash satisfying the difficulty target.  
- Core element of the **Proof-of-Work** process.

### 🧩 `align`
- Represents the **alignment index** (1 → CHILDREN).  
- Defines which branch (among siblings) this block belongs to.  
- Maintains balanced parallelism in the tree.

### 👷 `pqp_entry`
- A structure proving **block eligibility** via PQP (Parent Queue Pool).  
- Contains:
  - `queue_index` — position in the PQP traversal.(`bfs index in the tree`)
  - `miner_address` — the miner’s address (hex-encoded).
  - `prev_pqp_commitment` — links to the previous block of the same alignment (maintains chain of eligibility).
  - `signature` — miner’s ECDSA signature authenticating this entry.

### 🧾 `n_tx`
- Count of transactions within this block.  
- Used to verify completeness during block propagation and validation.

### 💰 `tx`
- Array of transaction objects (`Vec<Transaction>`).  
- Each transaction contributes to the **Merkle Root** and affects the **UTXO state**.

## ⚖️ Genesis Block

- The genesis block initializes the chain.  
- It has predefined values (e.g., static hash, `level = 0`, empty transactions).  
- The `miner_address` is set as `"GENISIS_LEADER_HEX"` — a symbolic placeholder for the first leader.

### 🧩 Hash & Commitment Workflow

1. **Calculate Block Hash**  
   Hash all block fields (including PQP entry and txids) → `block.hash`.

2. **Calculate PQP Commitment**  
   Hash PQP-related fields (`queue_index`,`align`,`hash`,`parent_hash`,`miner_address`,`prev_pqp_commitment`,`signature`) → `block.pqp_commitment`.

3. **Verification**  
   Each node can recompute both hashes locally to ensure:
   - No tampering with block content.
   - Valid link to PQP and parent chain.

### 🪜 Difficulty & Reward

- **Target Calculation:** Derived from `bits` (compact format → BigUint target).  
- **Difficulty Adjustment:**  
  - Every few blocks, the expected time (`EXPECTED_TIME`) is compared to actual mining time.  
  - Target adjusts within limits (¼×–4×) to maintain stability.
  - By default `BITS` is adjusted for every **CHILDREN * 100**
- **Reward Adjustment:**  
  - Block rewards halve every `HALVING_INTERVAL`.  
  - Initial subsidy defined by `INITIAL_SUBSIDY`.
  - By default reward is halved for every 1000 blocks.

#### 🧱 Example Block (Prototype JSON)
<details>
<summary>Click to view example</summary>
  
  ```json
  {
    "hash": "0000181c51c930a46ede1edbd3082c0e0d3673334fac3ddc60262c66a2c46b22",
    "pqp_commitment": "866d14c55b8e0523f53b9ba1e2b5e8554a859231f165bf6edb81f634d7ec22d7",
    "level": 0,
    "position": "0",
    "version": 1,
    "parent_hash": "0000000000000000000000000000000000000000000000000000000000000000",
    "merkle_root": "0000000000000000000000000000000000000000000000000000000000000000",
    "timestamp": 0,
    "bits": "1e1fffff",
    "nonce": 388736,
    "align": 0,
    "pqp_entry": {
      "queue_index": 0,
      "miner_address": "GENISIS_LEADER_HEX",
      "prev_pqp_commitment": "0000000000000000000000000000000000000000000000000000000000000000",
      "signature": ""
    },
    "n_tx": 0,
    "tx": []
  }
  ```
</details>

### Placeholder block
- Its a block with all empty feilds & 0s in numeric feilds
- Its used to be inserted into the treechain (index map) on behalf of the stale block.
- This prototype's place holder block size 482 bytes.
<details>
<summary>Click to view placeholder block</summary>
  
  ```json
  {
    "hash": "",
    "pqp_commitment": "",
    "level": 0,
    "position": "",
    "version": 0,
    "parent_hash": "",
    "merkle_root": "",
    "timestamp": 0,
    "bits": "",
    "nonce": 0,
    "align": 0,
    "pqp_entry": {
      "queue_index": 0,
      "miner_address": "",
      "prev_pqp_commitment": "",
      "signature": ""
    },
    "n_tx": 0,
    "tx": []
  }
  ```
</details>

---

# 🌴 TreeChain & ⛓ PQP

- **TreeChain** transforms the traditional *linear blockchain* (linked list) into a **tree-based structure**, enabling multiple blocks to grow concurrently.
- Structurally, it’s **not a recursive tree** — instead, it uses an **index map** to represent the hierarchy.
- The **index map** allows constant-time access (`O(1)`) to any block using either its `queue_index` or `hash`.
- This structure maintains the *tree logic* efficiently without deep recursive traversal.
- The **PQP (Parent Queue Pool)** acts as the linkage mechanism ensuring every new block is attached to a valid parent and a valid previous same aligned block, maintaining fairness in tree growth and parent order of blocks.
- When a block is missed because its parent already received children, it becomes **stale**.
- For every **stale or missing block**, a **placeholder block** is inserted into the index map to preserve structural integrity.
- As a result, both **block lookup** (by index or hash) and **tree traversal** remain **O(1)** operations, combining performance with structural accuracy.

## 🌳 TreeChain Structure

```rust
pub struct TreeChain {
    pub blocks: IndexMap<String, Block>,
    pub children_map: IndexMap<String, Vec<String>>,
    pub count: usize,
}
```

- **TreeChain** represents the complete **in-memory view** of the blockchain in **tree form**.  
- It maintains all blocks — both **valid** and **placeholders** — in efficient **hash-indexed maps** rather than a recursive node structure.

- **`blocks` → `IndexMap<String, Block>`**
  - Stores every block using its **hash** as the key.
  - Unlike a normal `HashMap`, an **IndexMap** preserves **insertion order**, so traversal can mimic **chronological** or **structural** order.
  - A block is inserted exactly at the index = block's queue_index
  - Provides **O(1)** lookup time for blocks and **predictable iteration order** — ideal for deterministic block trees.

- **`children_map` → `IndexMap<String, Vec<String>>`**
  - Maps each **parent block’s hash → list of its children block hashes**.
  - Maintains the **tree linkage** without recursive structs.
  - Enables quick retrieval of all children of a given parent in **constant time**.

- **`count` → `usize`**
  - Tracks only the **true (non-placeholder)** blocks within the chain.
  - **Placeholder blocks** (inserted for structural continuity when a block is missed) do **not** increment this count.

- Together, these maps let **TreeChain** act like a **logical tree**, but internally behave as an **index-based flat structure** — combining **structural clarity** with **O(1) access efficiency**.

### ⚙️ Workflow
- Unlike normal blockchains, **TreeChain** organizes blocks into a **series of aligned blocks**:
  - **1-aligned block**
  - **2-aligned block**
  - …
  - **CHILDREN-aligned block**

- Each **parent block** can have a maximum of **`CHILDREN` children blocks**.

- An **N-aligned block** must be mined such that:
  - Its **`parent_hash`** equals the **current parent’s**`(or next parent's -> when the same aligned block already exists for the current parent)` **block hash**.
  - Its **`prev_pqp_commitment`** equals the **PQP commitment** of the previous **N-aligned block**.
  - The **parent hash** ensures **tree structure**.
  - The **previous PQP commitment** ensures **chain continuity** — meaning hashing **never stops** across aligned blocks, making it **difficult to manipulate** data of any aligned block.

- After mining an **N-aligned block**:
  - The block is **added to the TreeChain**.
  - Its **PQP entry** is added to the **PQP**, along with:
    - The **alignment index**
    - The **block hash**
    - The **miner’s address**
- The **PQP (Parent Queue Pool)** represents an **ordered series of blocks** arranged by their **`queue_index`**, ensuring **parenting occurs in the correct order**.

- When an **N-aligned node** successfully mines the **N-aligned block** of the **current parent**:
  - That node immediately starts mining the **N-aligned block of the next parent**,  
    regardless of whether it has received **sibling blocks** of the current parent.
  - The **remaining aligned nodes** must finish mining their respective aligned blocks **before** this node finishes mining the next parent’s aligned block.  
    → This defines the **Parent-Child Completion Constraint**.

- Nodes mining blocks for the **current parent** should **immediately stop mining** when a block is received that is a **child of the next parent**.
  - At that moment, the **next parent becomes the new current parent**.
  - And all the nodes now mine on this new Parent.
  - This dynamic Parent switch is a **core feature of PQP** which ensures the compition not just among the same aligned nodes but also with all the other nodes

## 🎨 PQP (Parent Queue Pool)

- The **Parent Queue Pool (PQP)** maintains the **ordering and alignment** of all mined blocks in TreeChain.  
- It acts as a **queue**, allowing blocks to be appended from back and removed efficiently from front.

```rust
pub struct ParentQueueEntry {
    pub queue_index: u32,
    pub align: u8,
    pub block_hash: String,
    pub parent_hash: String,
    pub miner_address: String,
    pub prev_pqp_commitment: String,
    pub signature: String,
    pub pqp_commitment: String,
}

pub struct PQP {
    pub pool: Vec<ParentQueueEntry>,
}
```
- **`ParentQueueEntry`** represents a single entry in the **PQP**:

  - **`queue_index`** → Position of the block in the PQP sequence.  
  - **`align`** → Alignment level of the block (1-aligned, 2-aligned, …).  
  - **`block_hash`** → Hash of the block.  
  - **`parent_hash`** → Hash of the block’s parent.  
  - **`miner_address`** → Address of the node that mined the block.  
  - **`prev_pqp_commitment`** → Commitment of the previous aligned block.  
  - **`signature`** → Signature proving block validity.  
  - **`pqp_commitment`** → Final hash commitment for PQP linkage.  

- **`PQP`** holds all `ParentQueueEntry` instances inside its **`pool`**, which behaves as a **double-ended queue**.

- When a **new block** is created:
  - Its **PQP entry** (with additional metadata) is **added to the end** of the PQP pool.  
  - Initially, the PQP starts with only the **genesis entry**.  
  - The **first entry** in the PQP pool represents the **current parent**.  
  - The **second entry** represents the **next parent**.
 

- **Parent update conditions:**
  - When the **current parent** receives the maximum number of **`CHILDREN`** blocks (all pointing to its `block_hash`), it is **discarded**.  
  - If the **next parent** receives **any child block** (whose `parent_hash` equals the next parent’s `block_hash`):  
    - The **current parent** is **discarded**, and  
    - The **next parent** becomes the **new current parent**.
   
- **While creating a new block:**
  - It uses the **current parent’s `block_hash`** as its **`parent_hash`**.  
  - To determine **`prev_pqp_commitment`**:
    - It **iterates from the back** of the PQP pool, searching for an entry with the **same alignment**.  
    - If not found, it queries the **TreeChain (IndexMap)** starting from the **first entry’s `queue_index` → 0**,  
      until it finds a block of the same alignment.  
    - If found, it takes that block’s **`pqp_commitment`** as the **`prev_pqp_commitment`**.  
    - If no such aligned block exists, it defaults to the **genesis commitment**.
      
- **This mechanism ensures:**
  - **Order-preserving linkage** between aligned blocks.  
  - **Efficient parent updates** as the tree grows.  
  - **Continuous hashing** across all alignment levels — maintaining both **tree structure** and **sequential security**.  
---

