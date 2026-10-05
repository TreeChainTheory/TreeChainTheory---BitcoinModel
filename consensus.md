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
  - In a K-ary tree (e.g., a 3-ary tree, here K = CHILDREN), if all possible node positions are numbered sequentially from the genesis node (0) to the last node, from left to right and level by level, then each block’s queue_index equals the index of its intended position — even if one or more earlier positions (blocks) are missing.
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
- **Transaction Assignment (all transaction types):**
  - Every output belongs to the lane  
    `lane(output) = (first 64 bits of SHA-256(output.script_pubkey) % CHILDREN) + 1`
  - A transaction is valid only if **all its inputs** spend outputs of the same lane, and only a miner with `align == lane` can include it.
  - The same rule covers P2PKH, multisig (P2SH/P2WSH) and timelocked (CLTV/CSV) outputs: all outputs locked to one address share one lane.
- **Rollback Mechanism:**
  - Malicious or invalid subtrees can be pruned and valid parents re-queued.
  - _Currently disabled in the Bitcoin model; reserved for future versions._

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
  - By default `BITS` is adjusted for every **CHILDREN \* 100**
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

- **TreeChain** transforms the traditional _linear blockchain_ (linked list) into a **tree-based structure**, enabling multiple blocks to grow concurrently.
- Structurally, it’s **not a recursive tree** — instead, it uses an **index map** to represent the hierarchy.
- The **index map** allows constant-time access (`O(1)`) to any block using either its `queue_index` or `hash`.
- This structure maintains the _tree logic_ efficiently without deep recursive traversal.
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

# 👓 Visualization & Explanation

### Genisis Child Propagation (CHILDREN = 3)

<img width="450" height="300" alt="image" src="https://github.com/user-attachments/assets/d6b3b6e9-7018-4821-9f66-ccc8f4759316" />

- Initially There will be genesis in both Tree and PQPool acting as Parent and Previous Pqp commitment at the same tiem.
- When Some Node entered the **TREE** its immediately added to the **PQP**
- As soon as **Node 0** receives 3 children it is removed from the **PQPool**.

### 1st Node Parent

<img width="500" height="300" alt="image" src="https://github.com/user-attachments/assets/b601c0b8-647e-4726-a3c3-8fd12ab42cb8" />

- Each Node is supplies 2 feilds for next blocks , 1.**Parent Hash** , 2.**PQP commitment** .
- The **PQP** handles the order of blocks to be **parents**

### Next Prent getting Children earlier

<img width="600" height="341" alt="image" src="https://github.com/user-attachments/assets/a0c34da4-33b2-41b2-a280-e46780fd52c0" />

- Here **2** is **current parent** , but even beforet the 3rd aligned child is mined , 1st aligned child of **next parent** got mined. so the **next parent now becomes the current parent**.
- The current parent is now **removed** from the **PQPool**
- You can see the **queue_index** is preserved in the bfs left to right ordering even if a child is missed.

> **queue_index**: In a K-ary tree (e.g., a 3-ary tree, here K = CHILDREN), if all possible node positions are numbered sequentially from the genesis node (0) to the last node, from left to right and level by level, then each block’s queue_index equals the index of its intended position — even if one or more earlier positions (blocks) are missing.

### Prev PQP Commitment connection

<img width="600" height="381" alt="image" src="https://github.com/user-attachments/assets/27734001-a64b-4278-a6c3-1f111f69e78e" />

- The **PQP Commitment** connection ensures the chain continuity among different **aligned blocks** as shown in the diagram
- Its clear that the Previous PQP commitment is connected to the same aligned previous block in the **TREE**

> **Why PQP Commitment Connection is Needed**
>
> - Ensures **hash continuity** among different aligned blocks in the TreeChain.
> - Without this connection, when a block is mined, its hash is **not continued** until it becomes a parent block.
> - This breaks the cryptographic linkage (unlike Bitcoin’s continuous block hash chain).
> - Lack of continuity makes it **easier for attackers** to manipulate or rewrite parts of the TreeChain.
> - However in POS/POH the case is different but still the pqp commitment connection is needed `(but in different forms)`.

---

# 💸 Transactions

## 👉 **Key points**

- Follows the **same Bitcoin transaction cryptography and model**.
- Supports **P2PKH** and **Multisig** transactions.
- **Timelocked transactions** will be added in the future.
- Currently, there is **no witness support**.
- Fully compatible with **P2PKH transactions**, working exactly like Bitcoin.
- For **Multisig transactions (P2SH)**, this model supports **spending only one multisig UTXO at a time** (no multiple multisig UTXOs accepted).

## **Transaction Structure**

- Each transaction has:
  - `txid`
  - `vin`
  - `vout`
  - `witness`
- Every `vin` contains a `script_sig`, which holds the **signature + public key** just like bitcoin, and points to the output it spends.
- **Lane of an output:** the locking script (`script_pubkey`) of the output is hashed with SHA-256, and  
  `lane = (first 64 bits of the hash % CHILDREN) + 1`
- **Lane of a transaction:** the common lane of the outputs it spends. A node looks each spent output up in its UTXO set (or among unconfirmed transactions), rejects a transaction whose inputs fall into different lanes, and stores the lane with the transaction in the mempool. A miner with `align = a` selects only lane-`a` transactions, and every validator recomputes the lane of each transaction in a block.
- **Multisig and timelocked outputs** follow the same rule: all outputs locked to one multisig or timelock address share one lane, so they can be spent together.

- **Purpose of the Routing Rule**
  - Prevents **conflicting spends in parallel blocks**: two transactions that spend the same output always land in the same lane, because the output's locking script is the same for both. Blocks of different lanes, including siblings under one parent, can therefore never conflict.
  - Every node knows an output's lane **as soon as the output is created**, because the locking script is part of the output.

> **TreeChainTheory Transaction Routing**
>
> - **Normal token transfer:**
>   - Routed by the locking script of the coins being spent, i.e. by the sender's address.
> - **Smart contract creation:**
>   - Also routed by the sender's address.
> - **Smart contract interaction:**
>   - Token registration to a **contract address** is routed by the sender's address.
>   - Contract calls are routed by the **contract address** (SHA-256 of the address, `% CHILDREN + 1`) instead of the sender's address,  
>     but ensure **no transaction in that block** spends the same sender's coins.

- **Why the Locking Script Instead of the TxID or the Public Key's Last Digit**

  - If `txid` were used, a sender could spend multiple inputs and generate several transactions with **different lanes**,  
    which might all appear valid to different aligned miners.
  - The earlier rule (last hex digit of the sender's public key) only spread owners evenly when `CHILDREN` divides 16, could not tell an output's lane before it was spent, and needed a special case for multisig. Hashing the whole locking script fixes all three.
  - **Wallets with many keys** keep all their coins in one lane by only using addresses whose locking scripts map to their lane (about `CHILDREN` key generations per address). Wallets in this prototype hold a single key, so all their coins, including change, already share one lane.

- **Coinbase**:

  - Every block includes the 1st txn as **coinbase** just like bitcoin.
  - It cannot be spent untli **10 Blocks**.
  - The Halving interval is by default 1000 blocks.
    > This can be changed in `src/config.rs` file.

- **Summary**
  - The **UTXO , Transaction Pool & transaction model** is **exactly like Bitcoin**,  
    with additional alignment logic to ensure **non-repetition**, **fair miner distribution**, and **security against manipulation**.

---

# 𝌸 Constants

- **CHILDREN**

  - Maximum number of **child blocks per parent block**.
  - Can be changed as desired.
  - Example: `pub const CHILDREN: u8 = 3;`

- **BITS**

  - Defines the **default mining difficulty bits**.
  - Can be adjusted to make mining easier or harder.
  - Example:  
    `pub const BITS: &str = "1e1fffff";`
    - `1e1fffff` → Default
    - `1e0fffff` → 100× Harder
    - `1e7fffff` → 40× Harder
    - `1f0fffff` → 10× Harder
    - `207fffff` → Easier

- **INITIAL_SUBSIDY**

  - The **starting block reward** (in satoshis).
  - Example: `pub const INITIAL_SUBSIDY: u64 = 50 * 100_000_000;` → 50 BTC
  - Can be changed as required.

- **HALVING_INTERVAL**

  - Defines after how many blocks the **block reward halves**.
  - Example: `pub const HALVING_INTERVAL: u64 = 1000;` → Every 1000 blocks
  - Can be modified.

- **MINING_RATE**

  - Expected **average mining time per block** in milliseconds.
  - Example: `pub const MINING_RATE: i32 = 100_000;` → 100 seconds
  - Can be tuned for network performance.

- **EXPECTED_TIME**

  - Defines the **target timeframe** (in milliseconds) to produce  
    `CHILDREN * (EXPECTED_TIME / MINING_RATE)` blocks.
  - Example: `pub const EXPECTED_TIME: i128 = 10_000_000;` → 10,000 seconds
  - Can be modified as needed.
  - **Note:** Do not exceed `4,294,967,295` total queue index (u32 overflow).

- **Adjustment Rules**
  - For every `CHILDREN * (EXPECTED_TIME / MINING_RATE)` blocks → **Difficulty (BITS)** gets **adjusted**.
  - For every `HALVING_INTERVAL` blocks → **Block reward (SUBSIDY)** gets **halved**.

> 🤗 All The above constants **can be modified** in `src/config.rs` file.

- **Other Constants (Do Not Change)**
  - `INVMESSAGE_LIMIT: u16 = 40`
  - `GETDATA_LIMIT: u16 = 20`
  - `TESTING_WALLET_BALANCE: u64 = 500`
  - `USER_TXN_FREERATE: u64 = 3`
  - `SIGHASH_ALL: u32 = 0x01`
  - **These constants are system-defined and must not be altered.**

---

# 🛜 P2P Network

- **Overview**

  - The P2P layer forms the **communication backbone** of TreeChain, similar to Bitcoin’s peer-to-peer architecture.
  - Each node maintains **direct TCP connections** with its peers and exchanges blocks, transactions, and metadata asynchronously.
  - Built on **Tokio’s async runtime**, ensuring efficient non-blocking networking and scalability across many peers.

- **Node Setup**

  - Every node runs independently with the following environment variables:
    - `ALIGN` → Determines the alignment level of the node (e.g., 1, 2, 3, …).
    - `HTTP_PORT` → Used for REST/HTTP communication.
    - `P2P_PORT` → Used for peer-to-peer TCP connections.
  - Example startup commands:
    ```
    ALIGN=1 HTTP_PORT=3001 P2P_PORT=5001 cargo run --bin TreeChainTheorey
    ALIGN=2 HTTP_PORT=3002 P2P_PORT=5002 cargo run --bin TreeChainTheorey
    ALIGN=3 HTTP_PORT=3003 P2P_PORT=5003 cargo run --bin TreeChainTheorey
    ```

- **Connection to Ports Server**

  - Each node connects to a **central `ports_server`** running on a known port.
  - This server maintains a **registry of active peers** (IP, P2P_PORT, ALIGN).
  - On startup:
    - The node **registers itself** with the ports server.
    - Fetches the **list of all other peers** currently online.
    - Initiates TCP connections to synchronize with them.
  - This mechanism ensures **automatic peer discovery** and **network stability**, avoiding manual configuration.

- **Message Types**

  - Communication happens through **serialized JSON messages** using predefined message types:
    - `CONNECTION_INFO` → Handshake details and node metadata.
    - `REGISTER`, `PEER_LIST`, `UPDATE` → Peer registration and network updates.
    - `GETBLOCKS`, `BLOCK`, `MINED_BLOCK` → Block sharing and propagation.
    - `GETDATA`, `INVMESSAGE` → Request and announce block or transaction data.
    - `TRANSACTION`, `GET_TRANSACTION_POOL` → Transaction relay and synchronization.
    - `GET_PQP`, `PQP_RESPONSE` → Synchronization of Parent Queue Protocol (PQP) state.
    - `TREE_STATUS`, `GET_TREE`, `TREE` → Fork choice: tree summaries, request for a peer's tree, and the tree itself.

- **Synchronization & Behavior**

  - Continuously listens on `P2P_PORT` for incoming messages using `tokio::net::TcpListener`.
  - Uses **async read/write** (`AsyncReadExt`, `AsyncWriteExt`) for bidirectional communication.
  - Maintains a **peer list** in memory using `HashMap` and `Arc<Mutex>` for thread-safe access.
  - Periodically exchanges:
    - **Blocks** → For maintaining consistent TreeChain state.
    - **Transaction Pool** → For pending transactions.
    - **PQP Data** → For alignment continuity and block ordering.
  - Automatically re-attempts connection to dropped peers after short timeouts.

- **Fork Choice (longest tree)**

  - Nodes accept blocks in arrival order, so two nodes can briefly hold different trees (for example, when two miners of one lane fill the same slot).
  - Rule: the tree with **more blocks** wins; between trees with the same number of blocks, the one with the **smaller digest** wins (`TreeChain::tree_digest`: SHA-256 over the block hashes in slot order), so every node picks the same tree.
  - Every 5 s each node sends `TREE_STATUS` (block count and digest) to its peers. If a peer reports a better tree for 8 s, the node sends `GET_TREE` and receives `TREE`.
  - `TreeChain::rebuild_from_blocks` replays the received blocks from genesis in slot order through full validation. Only if that succeeds is the tree adopted: tree, PQP and UTXO set are swapped, transactions of dropped blocks return to the mempool, the miner pauses for 3 s, and missing blocks are fetched.
  - A block joins the tree only if the PQP accepted its entry, and a block that fails validation leaves the PQP unchanged, so a node's tree and PQP always match a replay of its blocks in slot order.

- **Bitcoin-Like Similarities**

  - Inspired by Bitcoin’s **Gossip Protocol**, where nodes relay blocks and transactions to connected peers.
  - Follows a similar **propagation model**, ensuring eventual consistency without central authority.
  - Uses **inventory (INV) messages** and **GETDATA requests** — same as Bitcoin — to prevent redundant data transfer.
  - Each node independently **validates** incoming blocks and transactions before relaying them further.
  - Ensures **decentralized consensus** and **fault tolerance** without relying on a single server (except the optional ports server for discovery).

- **Summary**
  - The TreeChain P2P layer is:
    - **Asynchronous**, **peer-synchronized**, and **self-healing**.
    - **Bitcoin-inspired** in protocol structure and validation flow.
    - **Extended** with alignment and PQP synchronization for TreeChain’s multi-aligned block model.

---

# 🧩 Conclusion

- **TreeChainTheorey** introduces a **parallelized blockchain structure**, increasing throughput while preserving Bitcoin’s Proof-of-Work security principles.
- The **Parent Queue Pool (PQP)** ensures fair and deterministic parent rotation, maintaining **alignment continuity** and **hash-chain integrity**.
- The system achieves **O(1) block access**, **balanced transaction alignment**, and **verifiable on-chain commitments**, delivering scalability without losing decentralization.

➡️ [Follow the steps to install and test TreeChainTheorey Bitcoin Model](./HowToRun.md)
