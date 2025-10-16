# 🚀 How to Run TreeChainTheorey – Bitcoin Model

---

## Step 1 — Prerequisites & Clone

- **Ensure you have Rust installed**

  - Install Rust:
    ```bash
    curl https://sh.rustup.rs -sSf | sh
    ```
  - Verify Rust:
    ```bash
    rustc --version
    ```

- **Ensure you have tmux installed**

  - Install tmux (macOS/Homebrew example):
    ```bash
    brew install tmux
    ```
  - Verify tmux:
    ```bash
    tmux -V
    ```

- **Clone the repository (main branch)**

  - Clone:
    ```bash
    git clone https://github.com/TreeChainTheory/TreeChainTheory---BitcoinModel
    ```
  - Enter project folder:
    ```bash
    cd TreeChainTheory---BitcoinModel
    ```

- **Confirm files exist (quick check)**
  - List top-level files:
    ```bash
    ls -la
    ```

## Step 2 — Project Structure & Build (bullet points)

- **Verify the folder structure**

  - You should see the following layout inside the project:
    ```
    TREECHAINTHEOREY
    ├── frontend/
    ├── src/
    ├── target/
    ├── .gitignore
    ├── Cargo.lock
    ├── Cargo.toml
    ├── consensus.md
    ├── errors.txt
    ├── HowToRun.md
    ├── multisigtxntest.txt
    ├── normalTxnTest.txt
    ├── README.md
    ├── starteverything_startmining.sh
    ├── startmining_all.sh
    ├── timelockedTxnTest.txt
    ├── tmux_run.sh
    ```

- **Build the project and fetch dependencies**

  - Run:
    ```bash
    cargo build
    ```

- **Verify successful build**
  - Check if `target/debug/` folder is created:
    ```bash
    ls target/debug
    ```

## Step 3 — Prepare and Run the Startup Script

- **Make Script Executable**

  - Give execute permission to the startup script:
    ```bash
    chmod +x starteverything_startmining.sh
    ```

- **Start Everything**

  - Run this command to start everything at once:
    ```bash
    ./starteverything_startmining.sh
    ```
  - **What this does:**
    - Starts the ports server:
    - Starts 6 backend nodes in tmux windows
    - Automatically triggers `/start_mining` on all nodes
    - Launches 6 frontend servers connected to the backends
    - Opens all frontend URLs in your browser

- **tmux Basic Controls**
  - **Detach from tmux** (leave everything running in background):
    - Press: `Ctrl + b`, then `d`
  - **Reattach to tmux session**:
    ```bash
    tmux attach -t tct
    ```
  - **Switch pages (windows)**:
    - Next window: `Ctrl + b`, then `n`
    - Previous window: `Ctrl + b`, then `p`
  - **Kill all sessions** (stop everything):
    ```bash
    tmux kill-session -t tct
    ```

## Step 4 — Frontend Access

- After the script runs:

  - Frontends will automatically open in your browser
  - Each frontend corresponds to a backend node

- **Example mapping:**

  - Frontend (port 5173) → Backend (port 3001)
  - Frontend (port 5174) → Backend (port 3002)
  - Frontend (port 5175) → Backend (port 3003)
  - Frontend (port 5176) → Backend (port 3004)
  - Frontend (port 5177) → Backend (port 3005)
  - Frontend (port 5178) → Backend (port 3006)

- **Use the frontend UI to:**
  - View blocks and PQP entries
  - Observe live mining activity
  - Create and broadcast All types of transactions

# Explore the frontend

---

- **Home page** -> Consists of all the **Overview** of the model
  <img width="400" height="313" alt="image" src="https://github.com/user-attachments/assets/8e11d694-d4a3-4094-b8ad-6a04a7b03ab0" />

---

- **Mining Page** -> Start/Stop mining , list of connected nodes , transaction pool & Utxo set
  <img width="400" height="313" alt="image" src="https://github.com/user-attachments/assets/94f5c5b8-200f-4667-8ff2-b971a967ad4b" />

---

- **Wallet Page** -> Get Balance , Create **Normal**, **Multisig** txns , Create **Unsigned** txn for multisig , **Sign**, **Spend**
  <img width="400" height="314" alt="image" src="https://github.com/user-attachments/assets/ca009e0a-d39e-4ba8-9f0b-ac5f96bd72a9" />

---

- **Tree Page** -> View the **Blocks** in **Queue Index** order , **Tree View** & **Level View** and Also look at the **PQP**
  <img width="400" height="312" alt="image" src="https://github.com/user-attachments/assets/48ef2c5f-c62b-4d1e-8640-80e45c5c924a" />

---

# Txns Testing ( P2PKH , P2SH -> m-of-n multisig)

## Create Normal P2PKH txn

- Paste wallet address of the receiver
- enter the amount and fee , send
- you can see the json response of the txn
  <img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/b78b764a-c398-4e13-abf8-e194d103f9c9" />
  <img width="400" height="320" alt="image" src="https://github.com/user-attachments/assets/0e46e4c2-77e1-46df-b712-26357e53a89f" />

## Create P2SH Mutlsig txn (m-of-n)

- Enter the pubkeys
- Enter the M (should not be more than N)
- Enter value and fee
  <img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/107fb178-d125-4436-9f81-1f7c6b4b5e49" />

- Find the txid by pasting the pubkeys and m
  <img width="400" height="150" alt="image" src="https://github.com/user-attachments/assets/d70d21a5-971f-4e4d-a18d-03a98f545aab" />

- response seems like this
  <img width="400" height="150" alt="image" src="https://github.com/user-attachments/assets/02e66c19-873f-4ebc-adcf-0e27678efca7" />

## Process of Spending the Multisig Utxo

- After Finding the Txid of the multisig txn
- Paste the Txid & vout
- Paste pubkeys , m
- Enter the Address , Value and Fee (Note: Make sure there is no remainder of the amount left in the multisig utxo)
  <img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/a066e2f0-1c9a-4dcd-98ca-eb31f45ce05f" />

- Copy Json of the unsigned Txns
  <img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/d74be1dd-5610-4cae-be35-d5e8be371392" />

- Get the Signatures of atleast m pubkeys
- Paste the Txid & vout of the spending multisig utxo
- Paste the Json Unsigned txn
- Paste the pubkey , m
  <img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/337aef21-1d19-453a-913e-94befe23f0b5" />

- Copy the signature
  <img width="400" height="150" alt="image" src="https://github.com/user-attachments/assets/060c6306-3acf-4bbf-a5f1-a3672269686c" />

- Paste the unsigned txn , pubkeys , m and the collected **Signatures** in Pubkeys order and click Spend
  <img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/3dd9e6f7-1cee-4dda-9737-60c58f94d0dc" />
  <img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/6f8f6c00-9fb7-4ae3-9c9a-a3320b09fe16" />

## Tree Page

- Click the **Tree View**
  <img width="400" height="312" alt="image" src="https://github.com/user-attachments/assets/48ef2c5f-c62b-4d1e-8640-80e45c5c924a" />

- Expand the view to view full tree
  <img width="400" height="313" alt="image" src="https://github.com/user-attachments/assets/451142af-b97c-42e9-8a68-9cb0b9d5be47" />

- You can also have a queue index order view
  <img width="400" height="312" alt="image" src="https://github.com/user-attachments/assets/c23feadf-25c2-4d6b-805f-012a494e77c3" />

- You can also have a level vise view
  <img width="400" height="160" alt="image" src="https://github.com/user-attachments/assets/f7879abf-96a9-4a04-b60a-6ccf0c04317c" />

- **PQP** View - have a look at the PQP (Parent Queue Pool)
  <img width="400" height="300" alt="image" src="https://github.com/user-attachments/assets/3c88c65a-a331-4b54-a04b-0d7bc8f7bc5e" />

## Access Through Backends (API Testing)

- To verify or interact directly with the backends using Postman or curl:

  - ⚙️ [Access Through Api→](./ApiAccess.md)

- **Description:**
  - This file includes all REST endpoints for backend nodes
  - Can be used to test:
    - `/start_mining`
    - `/get_blocks`
    - `/get_childrenmap`
    - `/create_txn`
    - etc.
  - Ideal for developers who want to inspect backend functionality manually
