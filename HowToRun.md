
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
    git clone https://github.com/TreeChainTheorey/TreeChainTheorey---BitcoinModel
    ```
  - Enter project folder:
    ```bash
    cd TreeChainTheorey---BitcoinModel
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
<img width="400" height="313" alt="image" src="https://github.com/user-attachments/assets/64d73639-642f-4e02-b531-7561e3f2d7f0" />

---
- **Tree Page** -> View the **Blocks** in **Queue Index** order , **Tree View** & **Level View** and Also look at the **PQP** 
<img width="400" height="313" alt="image" src="https://github.com/user-attachments/assets/93b2d985-ad71-4495-8355-070b42dc22b5" />

---
# Txns Testing ( P2PKH , P2SH -> m-of-n multisig)
## Create Normal P2PKH txn
- Paste wallet address of the receiver
- enter the amount and fee , send
- you can see the json response of the txn
<img width="400" height="313" alt="image" src="https://github.com/user-attachments/assets/89c74264-0381-4d96-a104-61198642b768" />

## Create P2SH Mutlsig txn (m-of-n)
- Enter the pubkeys
- Enter the M (should not be more than N)
- Enter value and fee
<img width="400" height="313" alt="image" src="https://github.com/user-attachments/assets/bbf46872-b033-4f80-90a3-c2a3790cb5da" />

- Find the txid by pasting the pubkeys and m
<img width="400" height="150" alt="image" src="https://github.com/user-attachments/assets/e8160a38-e96b-41a2-817e-e8b4378538a3" />

## Process of Spending the Multisig Utxo
- After Finding the Txid of the multisig txn
- Paste the Txid & vout
- Paste pubkeys , m
- Enter the Address , Value and Fee (Note: Make sure there is no remainder of the amount left in the multisig utxo)
<img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/de6fe76d-80ab-4286-ad42-6dd5acffa831" />

- Copy Json of the unsigned Txns
<img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/36589a82-62a6-44dc-b71e-8a325a16d944" />

- Get the Signatures of atleast m pubkeys
- Paste the Txid & vout of the spending multisig utxo
- Paste the Json Unsigned txn 
- Paste the pubkey , m
<img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/52669c10-a69b-4942-a997-09665deb6172" />

- Paste the unsigned txn , pubkeys , m and the collected **Signatures** and click Spend
<img width="400" height="310" alt="image" src="https://github.com/user-attachments/assets/ec41fee0-6488-4e3e-8d17-f628fad08f3b" />


## Access Through Backends (API Testing)

  - To verify or interact directly with the backends using Postman or curl:
    - See the file: `./ApiAccess.md`

  - **Description:**
    - This file includes all REST endpoints for backend nodes
    - Can be used to test:
      - `/start_mining`
      - `/get_chain`
      - `/send_txn`
      - `/get_utxos`
      - etc.
    - Ideal for developers who want to inspect backend functionality manually

