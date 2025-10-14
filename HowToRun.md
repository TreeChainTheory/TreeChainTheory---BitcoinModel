
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
    - Create and broadcast transactions

## Access Through Backends (API Testing)

  - To verify or interact directly with the backends using Postman or curl:
    - See the file: `accessThroughBackends.md`

  - **Description:**
    - This file includes all REST endpoints for backend nodes
    - Can be used to test:
      - `/start_mining`
      - `/get_chain`
      - `/send_txn`
      - `/get_utxos`
      - etc.
    - Ideal for developers who want to inspect backend functionality manually

