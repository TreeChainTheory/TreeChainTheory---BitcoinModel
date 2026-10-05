#!/usr/bin/env python3
"""Local 6-node run of the TreeChainTheory PoW prototype (N = CHILDREN = 3).

Mirrors starteverything_startmining.sh (ports server + 6 backend nodes with
ALIGN 1,2,3,1,2,3, then /start_mining on every node) without tmux or the
frontends. While the nodes mine, it submits real P2PKH transactions between
the node wallets through the public /create_txn endpoint (closed loop: each
wallet has at most one unconfirmed payment at a time) and snapshots every
node's view every 30 s. At the end it stops mining, dumps each node's final
state, and shuts the processes down.

Usage: run_local_testbed.py <repo_dir> <out_dir> [mining_seconds]
"""
import json
import os
import random
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request

REPO = sys.argv[1]
OUT = sys.argv[2]
MINING_SECONDS = int(sys.argv[3]) if len(sys.argv) > 3 else 1500
NODES = [
    # (node id, ALIGN, HTTP port, P2P port) -- same layout as the tmux script
    (1, 1, 3001, 5001),
    (2, 2, 3002, 5002),
    (3, 3, 3003, 5003),
    (4, 1, 3004, 5004),
    (5, 2, 3005, 5005),
    (6, 3, 3006, 5006),
]
# lane-aware wallets: each node's wallet uses an address in this lane (two wallets per lane, and
# never the node's own mining lane, so every payment is mined by other nodes); None = random key
WALLET_LANES = {1: 2, 2: 3, 3: 1, 4: 2, 5: 3, 6: 1}
TX_INTERVAL = 4.0          # seconds between transaction rounds
SNAPSHOT_INTERVAL = 30.0   # seconds between state snapshots
TX_FEE = 10_000            # satoshis (well above the 3 sat/vB floor)

os.makedirs(os.path.join(OUT, "logs"), exist_ok=True)
os.makedirs(os.path.join(OUT, "final"), exist_ok=True)
procs = []


def log(msg):
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


def http_get(port, path, timeout=60):
    url = f"http://127.0.0.1:{port}{path}"
    with urllib.request.urlopen(url, timeout=timeout) as resp:
        return json.loads(resp.read().decode())


def http_post(port, path, body, timeout=60):
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, json.loads(resp.read().decode())
    except urllib.error.HTTPError as err:
        try:
            return err.code, json.loads(err.read().decode())
        except Exception:
            return err.code, {"error": str(err)}
    except Exception as err:  # connection problems, timeouts
        return 0, {"error": repr(err)}


def wait_for_tcp(port, timeout=60):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=1):
                return True
        except OSError:
            time.sleep(0.3)
    return False


def start_processes():
    # TESTBED_BUILD=release runs the optimized build (much faster hashing); default: debug
    bin_dir = os.path.join(REPO, "target", os.environ.get("TESTBED_BUILD", "debug"))
    ports_log = open(os.path.join(OUT, "logs", "ports_server.log"), "w")
    procs.append(subprocess.Popen([os.path.join(bin_dir, "ports_server")], cwd=REPO,
                                  stdout=ports_log, stderr=subprocess.STDOUT))
    if not wait_for_tcp(8080):
        raise RuntimeError("ports server did not start on :8080")
    log("ports server up on :8080")
    for node_id, align, http_port, p2p_port in NODES:
        env = dict(os.environ, ALIGN=str(align), HTTP_PORT=str(http_port), P2P_PORT=str(p2p_port))
        if WALLET_LANES and WALLET_LANES.get(node_id):
            env["WALLET_LANE"] = str(WALLET_LANES[node_id])
        # fork test only: TESTBED_ISOLATE="node:start:duration" cuts that node off from incoming
        # blocks for `duration` seconds, starting `start` seconds after it starts
        iso = os.environ.get("TESTBED_ISOLATE", "")
        if iso and iso.split(":")[0] == str(node_id):
            env["TEST_ISOLATE"] = ":".join(iso.split(":")[1:])
        node_log = open(os.path.join(OUT, "logs", f"node{node_id}.log"), "w")
        procs.append(subprocess.Popen([os.path.join(bin_dir, "TreeChainTheorey")], cwd=REPO,
                                      env=env, stdout=node_log, stderr=subprocess.STDOUT))
        time.sleep(0.5)  # same stagger as the tmux script
    for node_id, _, http_port, _ in NODES:
        if not wait_for_tcp(http_port):
            raise RuntimeError(f"node {node_id} HTTP port {http_port} never opened")
    log("all 6 node HTTP servers up")


def stop_processes():
    for proc in procs:
        if proc.poll() is None:
            proc.send_signal(signal.SIGTERM)
    time.sleep(2)
    for proc in procs:
        if proc.poll() is None:
            proc.kill()


def snapshot(t0, fh):
    now = time.time()
    rows = []
    for node_id, align, http_port, _ in NODES:
        row = {"t_s": round(now - t0, 1), "node": node_id, "align": align,
               "process_alive": procs[node_id].poll() is None}  # procs[0] is the ports server
        try:
            started = time.time()
            blocks = http_get(http_port, "/get_blocks")["blocks"]
            row["get_blocks_ms"] = round((time.time() - started) * 1000)
            real = [b for b in blocks if b.get("position")]
            row["real_blocks"] = len(real)
            row["max_queue_index"] = max(b["pqp_entry"]["queue_index"] for b in real)
            row["user_txs_in_blocks"] = sum(max(b["n_tx"] - 1, 0) for b in real)
            row["latest_bits"] = real[-1]["bits"] if real else None
            row["block_set_digest"] = hash(tuple(sorted(b["hash"] for b in real))) & 0xFFFFFFFF
            row["pqp_len"] = len(http_get(http_port, "/get_pqp")["blocks"])
            row["mempool"] = http_get(http_port, "/transaction_pool").get("pool_size")
            row["peers"] = len(http_get(http_port, "/get_connected_peers")["peers"])
        except Exception as err:
            row["error"] = repr(err)
        rows.append(row)
        fh.write(json.dumps(row) + "\n")
    fh.flush()
    agree = len({r.get("block_set_digest") for r in rows}) == 1
    counts = [r.get("real_blocks") for r in rows]
    alive = sum(r["process_alive"] for r in rows)
    errors = sum("error" in r for r in rows)
    log(f"t={rows[0]['t_s']:>6}s blocks per node={counts} identical_views={agree} "
        f"alive={alive}/6 http_errors={errors} bits={rows[0].get('latest_bits')}")


def dump_final():
    endpoints = {
        "blocks": "/get_blocks",
        "pqp": "/get_pqp",
        "children_map": "/children_map",
        "wallet": "/wallet",
        "utxo_set": "/utxo_set",
        "mempool": "/transaction_pool",
        "peers": "/get_connected_peers",
        "verify_pqp": "/verify_pqp",
        "verify_tree": "/verify_tree",
    }
    for node_id, _, http_port, _ in NODES:
        for name, path in endpoints.items():
            try:
                data = http_get(http_port, path, timeout=600)
            except Exception as err:
                data = {"error": repr(err)}
            with open(os.path.join(OUT, "final", f"{name}_node{node_id}.json"), "w") as fh:
                json.dump(data, fh)
        log(f"final state dumped for node {node_id}")


def main():
    meta = {"started_at": time.strftime("%Y-%m-%d %H:%M:%S %Z"), "mining_seconds": MINING_SECONDS,
            "build": os.environ.get("TESTBED_BUILD", "debug"),
            "isolation_test": os.environ.get("TESTBED_ISOLATE") or None,
            "nodes": NODES, "tx_interval_s": TX_INTERVAL, "tx_fee_sat": TX_FEE,
            "workload": "closed loop, at most one unconfirmed payment per wallet",
            "wallet_lanes_requested": WALLET_LANES}
    git = lambda *args: subprocess.run(["git", *args], cwd=REPO, capture_output=True, text=True).stdout.strip()
    meta["code"] = {"head": git("rev-parse", "--short", "HEAD"),
                    "uncommitted_src_changes": git("diff", "--name-only", "HEAD", "--", "src").split()}
    start_processes()
    time.sleep(3)  # same settle time as the tmux script before /start_mining
    peer_counts = {}
    for node_id, _, http_port, _ in NODES:
        peer_counts[node_id] = len(http_get(http_port, "/get_connected_peers")["peers"])
    log(f"connected peers before mining: {peer_counts}")
    meta["peers_before_mining"] = peer_counts

    wallets = {}
    for node_id, align, http_port, _ in NODES:
        info = http_get(http_port, "/wallet")["wallet"]
        pubkey = info["public_key"]
        # nodes that report their lane use the locking-script rule; older builds routed by the
        # last hex digit of the public key
        lane = info.get("lane") or int(pubkey[-1], 16) % 3 + 1
        wallets[node_id] = {"address": info["address"], "public_key": pubkey, "align": align,
                            "tx_lane": lane}
    meta["wallets"] = wallets
    meta["routing_rule"] = ("hash of the locking script" if "lane" in info
                            else "last hex digit of the public key")
    log(f"sender lanes ({meta['routing_rule']}): "
        + ", ".join(f"node{n}->lane{w['tx_lane']}" for n, w in wallets.items()))

    for node_id, _, http_port, _ in NODES:
        log(f"start_mining node {node_id}: {http_get(http_port, '/start_mining')}")
    t0 = time.time()
    meta["mining_started_epoch_ms"] = int(t0 * 1000)

    tx_fh = open(os.path.join(OUT, "tx_log.jsonl"), "w")
    tl_fh = open(os.path.join(OUT, "timeline.jsonl"), "w")
    next_snapshot = t0
    sent = ok = 0
    pending = {n: None for n, _, _, _ in NODES}  # txid of each wallet's unconfirmed payment
    while time.time() - t0 < MINING_SECONDS:
        if time.time() >= next_snapshot:
            snapshot(t0, tl_fh)
            next_snapshot += SNAPSHOT_INTERVAL
        order = NODES[:]
        random.shuffle(order)
        for node_id, _, http_port, _ in order:
            if pending[node_id]:
                try:
                    pool = http_get(http_port, "/transaction_pool", timeout=30)
                    in_pool = {t.get("txid") for t in pool.get("transactions", [])}
                except Exception:
                    in_pool = {pending[node_id]}
                if pending[node_id] in in_pool:
                    continue  # previous payment not confirmed yet: one payment in flight per wallet
                pending[node_id] = None
            others = [w["address"] for n, w in wallets.items() if n != node_id]
            value = random.randint(10_000_000, 100_000_000)  # 0.1 - 1 BTC
            status, resp = http_post(http_port, "/create_txn",
                                     {"to_address": random.choice(others), "value": value, "fee": TX_FEE})
            sent += 1
            success = bool(resp.get("success"))
            ok += success
            if success:
                pending[node_id] = resp.get("txid")
            tx_fh.write(json.dumps({"t_s": round(time.time() - t0, 2), "node": node_id, "status": status,
                                    "success": success, "txid": resp.get("txid"),
                                    "error": None if success else str(resp.get("error"))[:200]}) + "\n")
        tx_fh.flush()
        time.sleep(TX_INTERVAL)
    log(f"mining window over: {ok}/{sent} transaction submissions accepted into mempools")

    for node_id, _, http_port, _ in NODES:
        try:
            log(f"stop_mining node {node_id}: {http_get(http_port, '/stop_mining')}")
        except Exception as err:
            log(f"stop_mining node {node_id} failed: {err!r}")
    time.sleep(30)  # let in-flight blocks propagate and the fork choice settle (status every 5 s, 8 s grace)
    snapshot(t0, tl_fh)
    meta["mining_stopped_epoch_ms"] = int(time.time() * 1000)
    dump_final()
    meta["finished_at"] = time.strftime("%Y-%m-%d %H:%M:%S %Z")
    with open(os.path.join(OUT, "meta.json"), "w") as fh:
        json.dump(meta, fh, indent=2)


if __name__ == "__main__":
    try:
        main()
    finally:
        stop_processes()
        log("all node processes stopped")
