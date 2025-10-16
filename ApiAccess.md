# API Access Guide

## 🚀 How to Start

**Start Ports Server**
   ```bash
   cargo run --bin ports_server
   ```

## Start TreeChain Nodes (Minimum 3 Nodes Required)

- **1st terminal**
  - ```bash
    ALIGN=1 HTTP_PORT=3001 P2P_PORT=5001 cargo run --bin TreeChainTheorey
    ```
- **2nd terminal**
  - ```bash
    ALIGN=2 HTTP_PORT=3002 P2P_PORT=5002 cargo run --bin TreeChainTheorey
    ```
- **3rd terminal**
  - ```bash
    ALIGN=3 HTTP_PORT=3003 P2P_PORT=5003 cargo run --bin TreeChainTheorey
    ```
- **4th terminal**
  - ```bash
    ALIGN=1 HTTP_PORT=3004 P2P_PORT=5004 cargo run --bin TreeChainTheorey
    ```
- **5th terminal**
  - ```bash
    ALIGN=2 HTTP_PORT=3005 P2P_PORT=5005 cargo run --bin TreeChainTheorey
    ```
- **6th terminal**
  - ```bash
    ALIGN=3 HTTP_PORT=3006 P2P_PORT=5006 cargo run --bin TreeChainTheorey
    ```

# 🧱 Mining API Endpoints

- **`/start_mining`**
  - **Description:** Starts the mining process for the node.
  - **Response:**
    ```json
    {
      "message": "Mining started",
      "status": "success"
    }
    ```

- **`/stop_mining`**
  - **Description:** Stops the mining process for the node.
  - **Response:**
    ```json
    {
      "message": "Mining stopped",
      "status": "success"
    }
    ```

- **`/get_blocks`**
  - **Description:** Retrieves the list of mined blocks in `queue_index` order.
  - **Response Example:**
    ```json
    {
      "blocks": [
        {
          "align": 0,
          "bits": "1e7fffff",
          "hash": "0000181c51c930a46ede1edbd3082c0e0d3673334fac3ddc60262c66a2c46b22",
          "level": 0,
          "merkle_root": "0000000000000000000000000000000000000000000000000000000000000000",
          "n_tx": 0,
          "nonce": 388736,
          "parent_hash": "0000000000000000000000000000000000000000000000000000000000000000",
          "position": "0",
          "pqp_commitment": "866d14c55b8e0523f53b9ba1e2b5e8554a859231f165bf6edb81f634d7ec22d7",
          "pqp_entry": {
            "miner_address": "GENISIS_LEADER_HEX",
            "prev_pqp_commitment": "0000000000000000000000000000000000000000000000000000000000000000",
            "queue_index": 0,
            "signature": ""
          },
          "timestamp": 0,
          "tx": [],
          "version": 1
        }
      ]
    }
    ```

- **`/wallet`**
  - **Description:** Returns detailed wallet information including balance, UTXOs, and metadata.
  - **Response Example:**
    ```json
    {
      "metadata": {
        "node_version": "0.1.0",
        "timestamp": "2025-10-16T04:33:07.703721+00:00",
        "utxo_set_height": 396
      },
      "security": {
        "address_verification": "valid",
        "derived_pubkey_hash": "a9243aefc7c9b30e3b75303aae1195bc5b791ef1",
        "expected_pubkey_hash": "a9243aefc7c9b30e3b75303aae1195bc5b791ef1",
        "wallet_format": "simple"
      },
      "transaction_stats": {
        "net_balance_satoshis": 33397321681,
        "total_received_btc": "333.97321681",
        "total_received_satoshis": 33397321681,
        "total_sent_btc": "0.00000000",
        "total_sent_satoshis": 0
      },
      "utxos": {
        "count": 66,
        "dust_utxos": 0,
        "total_value_satoshis": 33397321681
      },
      "wallet": {
        "address": "1GRLbjspgWJSmNEd922b8APP2Ch3JtmPNS",
        "balance_btc": "333.97321681",
        "balance_satoshis": {
          "total": 33397321681
        },
        "public_key": "03d696b22e5cb8109c7762fab20ef8dc3841cdddacd842616884d0b19b343a0ade",
        "public_key_hash": "a9243aefc7c9b30e3b75303aae1195bc5b791ef1"
      },
      "warnings": []
    }
    ```

# 💸 Transactions
---

## 🔹 P2PKH Normal Transactions

- **Endpoint:** `/create_transaction`
  - **Description:** Creates and broadcasts a standard Pay-to-Public-Key-Hash (P2PKH) transaction.
  - **Request Body:**
    ```json
    {
      "to_address": "19ZCMN2d2LQbrqzMbzoUQUriaWUukgx8mA", 
      "value": 5699999999,
      "fee": 100000000
    }
    ```
  - **Response Example:**
    ```json
    {
      "fee": 100000000,
      "from_address": "1Em1o7Dpgh9tFPSXc7A94arw95a1BsyWct",
      "success": true,
      "to_address": "19ZCMN2d2LQbrqzMbzoUQUriaWUukgx8mA",
      "total_input": 5799999999,
      "transaction": {
        "hash": "27cb598f207b54c4bf0a7dadf87abbf938dda9c6833fda86ad625ee32c3833be",
        "locktime": 0,
        "txid": "27cb598f207b54c4bf0a7dadf87abbf938dda9c6833fda86ad625ee32c3833be",
        "version": 1,
        "vin": [
          {
            "script_sig": "473045022100ac049092cf77f4a1bfb6fb759e4e088eeafad4012519d8a38f09a354f2cc3fbb022010bda1d3a47da54dbc795c2bfa361429358c964824e6d119fc8592d0d4f5aee521026c469f4feb872c4d950e15c1ece80993a5a10d5cc1f7e1c7385d2836b05bb87a",
            "sequence": 4294967295,
            "txid": "26b4cb81ff8d04fd24678081b034132cd3fbbe637178d0e3637c2dfa04a5c98b",
            "vout": 0
          },
          {
            "script_sig": "46304402204ab0c8189a318489769782b683e12a23acc9a48337af0a6a78674c6cd866055d02203794d7b2236d946b1d25df3cc5d6a06062efe9411299a6a9d92d4aad9df6f31921026c469f4feb872c4d950e15c1ece80993a5a10d5cc1f7e1c7385d2836b05bb87a",
            "sequence": 4294967295,
            "txid": "d2950c0d08369b140b9408d5bd43d68f2cac96e387b407abd4f9dc8c9af92bd2",
            "vout": 0
          }
        ],
        "vout": [
          {
            "script_pubkey": "76a9145dd7cade0ae535c7df0f586cd8f081f59faf640d88ac",
            "value": 5699999999
          },
          {
            "script_pubkey": "76a91496ec99e1c0f7ea3c187e34d378267da25f58075a88ac",
            "value": 4200000001
          }
        ],
        "witnesses": null
      },
      "txid": "27cb598f207b54c4bf0a7dadf87abbf938dda9c6833fda86ad625ee32c3833be",
      "value": 5699999999,
      "wallet_balance": 94200000001
    }
    ```
## 🔐 Create Multisig Transactions

### 🧩 Overview (example usage)
Node 6 created the multisig transaction using the public keys of Node 4, Node 5, and Node 6 (`m = 2`), sending **10 BTC + 1 BTC fee**.  
The following steps outline the process of creating, signing, and spending a multisig transaction.

---

- **Step 1: `/create_multisig_txn`**
  - **Description:** Create a new multisig transaction with multiple public keys and required signatures (`m-of-n` scheme).
  - **Request Body:**
    ```json
    {
      "pubkeys": [
        "02caab3f698f17af016290ba6258b77eacc162397d8f4bfd020a333c44abc3d0e1",
        "02a6960aec3193d6e6ff39af59eb0c82149b2634f6973a3d8e063c2ea573f28f96",
        "03072270e9d620044e515b3e4810c68725ec2fd1ea1f43235fa571dcd4dc9899f2"
      ],
      "m": 2,
      "value": 1000000000,
      "fee": 100000000
    }
    ```
  - **Response Example:**
    ```json
    {
      "status": "success",
      "transaction": {
        "hash": "4fdbb2c590ac6fa81649ef8043088a3e88f74dcb0750413c4296733148c17216",
        "locktime": 0,
        "txid": "4fdbb2c590ac6fa81649ef8043088a3e88f74dcb0750413c4296733148c17216",
        "version": 1,
        "vin": [
          {
            "script_sig": "463044022075bc2081896c268eb34793d6c5efef73a6de4d6d944145e60c9a6c094282c3150220754b5b2a08890bf6cc07a7fefda225ecc1f14ba548a06b88b0f127e7a28a411c2103072270e9d620044e515b3e4810c68725ec2fd1ea1f43235fa571dcd4dc9899f2",
            "sequence": 4294967295,
            "txid": "dbd8339a12ea1771d8961e73043776769c40eede47d3bf1607e725595798d9ab",
            "vout": 0
          }
        ],
        "vout": [
          {
            "script_pubkey": "a914cafa11010c3f1a6575f3c5dea57a4c366a33fa4687",
            "value": 1000000000
          },
          {
            "script_pubkey": "76a91496030e7179472346962a124f4d7f3efb5c58dd1b88ac",
            "value": 3900000000
          }
        ],
        "witnesses": null
      }
    }
    ```

---

- **Step 2: `/create_spending_multisig_tx`**
  - **Description:** Create an unsigned transaction to spend from the multisig UTXO. Ensure no remainder amount is left.
  - **Request Body:**
    ```json
    {
      "txid": "4fdbb2c590ac6fa81649ef8043088a3e88f74dcb0750413c4296733148c17216",
      "vout": 0,
      "pubkeys": [
        "02caab3f698f17af016290ba6258b77eacc162397d8f4bfd020a333c44abc3d0e1",
        "02a6960aec3193d6e6ff39af59eb0c82149b2634f6973a3d8e063c2ea573f28f96",
        "03072270e9d620044e515b3e4810c68725ec2fd1ea1f43235fa571dcd4dc9899f2"
      ],
      "m": 2,
      "to_address": "1D3UieWnj83RhytAWSugNqRSaf3ndPGE4s",
      "value": 900000000,
      "fee": 100000000
    }
    ```
  - **Response Example:**
    ```json
    {
      "status": "success",
      "unsigned_transaction": {
        "hash": "cf74135139cfa19b4793dbf84420a7a272144877ccfdf28fa9026755369b680d",
        "txid": "cf74135139cfa19b4793dbf84420a7a272144877ccfdf28fa9026755369b680d",
        "version": 1,
        "vin": [
          {
            "script_sig": "",
            "sequence": 4294967295,
            "txid": "4fdbb2c590ac6fa81649ef8043088a3e88f74dcb0750413c4296733148c17216",
            "vout": 0
          }
        ],
        "vout": [
          {
            "script_pubkey": "76a91484197aac356ba42ce71500c90ced3d14c5fea9c188ac",
            "value": 900000000
          }
        ],
        "witnesses": null
      }
    }
    ```

---

- **Step 3: `/sign_multisig`**
  - **Description:** Each participant (corresponding to a pubkey) signs the unsigned multisig transaction.
  - **Request Body:**
    ```json
    {
      "txid": "4fdbb2c590ac6fa81649ef8043088a3e88f74dcb0750413c4296733148c17216",
      "vout": 0,
      "spending_tx": {
        "hash": "cf74135139cfa19b4793dbf84420a7a272144877ccfdf28fa9026755369b680d",
        "txid": "cf74135139cfa19b4793dbf84420a7a272144877ccfdf28fa9026755369b680d",
        "version": 1,
        "vin": [
          {
            "script_sig": "",
            "sequence": 4294967295,
            "txid": "4fdbb2c590ac6fa81649ef8043088a3e88f74dcb0750413c4296733148c17216",
            "vout": 0
          }
        ],
        "vout": [
          {
            "script_pubkey": "76a91484197aac356ba42ce71500c90ced3d14c5fea9c188ac",
            "value": 900000000
          }
        ],
        "witnesses": null
      },
      "pubkeys": [
        "02caab3f698f17af016290ba6258b77eacc162397d8f4bfd020a333c44abc3d0e1",
        "02a6960aec3193d6e6ff39fa26c9b76f52951b840e9f17d89ec0a7c7f87d71844022019d265ef97c4c3f5ce21295b658588911a916ed236e7d03ed6ab6efd50b51719",
        "03072270e9d620044e515b3e4810c68725ec2fd1ea1f43235fa571dcd4dc9899f2"
      ],
      "m": 2
    }
    ```
  - **Signatures:**
    - **Signature 1:**
      ```json
      {
        "signature": "3045022100afed1354eb068b9e8d309840125f71ba76c8c088f028b3f9aec180845f701c5d0220595dba8be1d16e6a2956e994b0cb658bf963ac151998027952303485a3099a72",
        "status": "success"
      }
      ```
    - **Signature 2:**
      ```json
      {
        "signature": "30450221009eec3c9b4d5995e6aff39fa26c9b76f52951b840e9f17d89ec0a7c7f87d71844022019d265ef97c4c3f5ce21295b658588911a916ed236e7d03ed6ab6efd50b51719",
        "status": "success"
      }
      ```

---

- **Step 4: `/spend_multisig_txn`**
  - **Description:** Combine collected signatures in correct pubkey order to broadcast the final signed transaction.
  - **Request Body:**
    ```json
    {
      "spending_tx": {
        "hash": "cf74135139cfa19b4793dbf84420a7a272144877ccfdf28fa9026755369b680d",
        "txid": "cf74135139cfa19b4793dbf84420a7a272144877ccfdf28fa9026755369b680d",
        "version": 1,
        "vin": [
          {
            "script_sig": "",
            "sequence": 4294967295,
            "txid": "4fdbb2c590ac6fa81649ef8043088a3e88f74dcb0750413c4296733148c17216",
            "vout": 0
          }
        ],
        "vout": [
          {
            "script_pubkey": "76a91484197aac356ba42ce71500c90ced3d14c5fea9c188ac",
            "value": 900000000
          }
        ],
        "witnesses": null
      },
      "pubkeys": [
        "02caab3f698f17af016290ba6258b77eacc162397d8f4bfd020a333c44abc3d0e1",
        "02a6960aec3193d6e6ff39fa26c9b76f52951b840e9f17d89ec0a7c7f87d71844022019d265ef97c4c3f5ce21295b658588911a916ed236e7d03ed6ab6efd50b51719",
        "03072270e9d620044e515b3e4810c68725ec2fd1ea1f43235fa571dcd4dc9899f2"
      ],
      "m": 2,
      "sigs": [
        "3045022100afed1354eb068b9e8d309840125f71ba76c8c088f028b3f9aec180845f701c5d0220595dba8be1d16e6a2956e994b0cb658bf963ac151998027952303485a3099a72",
        "30450221009eec3c9b4d5995e6aff39fa26c9b76f52951b840e9f17d89ec0a7c7f87d71844022019d265ef97c4c3f5ce21295b658588911a916ed236e7d03ed6ab6efd50b51719"
      ]
    }
    ```
  - **Response Example:**
    ```json
    {
      "fee": 100000000,
      "from_multisig": "4fdbb2c590ac6fa81649ef8043088a3e88f74dcb0750413c4296733148c17216:0",
      "num_signatures": 2,
      "required_signatures": 2,
      "success": true,
      "total_input": 1000000000,
      "total_output": 900000000,
      "transaction": {
        "hash": "35003179e4faccfed3234065bbec522cf61afc3d94045bfe0ee98023cf251be1",
        "txid": "35003179e4faccfed3234065bbec522cf61afc3d94045bfe0ee98023cf251be1",
        "version": 1,
        "vin": [
          {
            "script_sig": "00473045022100afed1354eb068b9e8d309840125f71ba76c8c088f028b3f9aec180845f701c5d0220595dba8be1d16e6a2956e994b0cb658bf963ac151998027952303485a3099a724730450221009eec3c9b4d5995e6aff39fa26c9b76f52951b840e9f17d89ec0a7c7f87d71844022019d265ef97c4c3f5ce21295b658588911a916ed236e7d03ed6ab6efd50b5171969522102caab3f698f17af016290ba6258b77eacc162397d8f4bfd020a333c44abc3d0e12102a6960aec3193d6e6ff39af59eb0c82149b2634f6973a3d8e063c2ea573f28f962103072270e9d620044e515b3e4810c68725ec2fd1ea1f43235fa571dcd4dc9899f253ae",
            "sequence": 4294967295,
            "txid": "4fdbb2c590ac6fa81649ef8043088a3e88f74dcb0750413c4296733148c17216",
            "vout": 0
          }
        ],
        "vout": [
          {
            "script_pubkey": "76a91484197aac356ba42ce71500c90ced3d14c5fea9c188ac",
            "value": 900000000
          }
        ],
        "witnesses": null
      },
      "txid": "35003179e4faccfed3234065bbec522cf61afc3d94045bfe0ee98023cf251be1"
    }
    ```
