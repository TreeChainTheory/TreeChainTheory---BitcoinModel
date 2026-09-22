#!/usr/bin/env python3
"""Analyze the local 6-node run produced by run_local_testbed.py.

Reads meta.json, timeline.jsonl, tx_log.jsonl, final/*.json and logs/*.log in
the given directory and prints a JSON summary used to write results.md.
"""
import bisect
import collections
import gzip
import json
import os
import re
import statistics
import sys

D = sys.argv[1] if len(sys.argv) > 1 else "."
N = 3


def load(name):
    with open(os.path.join(D, name)) as fh:
        return json.load(fh)


def lane_of_pubkey_hex(h):
    return int(h[-1], 16) % N + 1


meta = load("meta.json")
t0_ms = meta["mining_started_epoch_ms"]
out = {"meta": {k: meta[k] for k in ("started_at", "finished_at", "mining_seconds", "peers_before_mining")},
       "sender_lanes": {n: w["tx_lane"] for n, w in meta["wallets"].items()},
       "miner_lanes": {str(n): a for n, a, _, _ in meta["nodes"]}}

# ---- final per-node views ----------------------------------------------------
views = {}
for node in range(1, 7):
    try:
        blocks = load(f"final/blocks_node{node}.json").get("blocks")
    except Exception:
        blocks = None
    if blocks:
        views[node] = [b for b in blocks if b.get("position")]
out["nodes_with_final_state"] = sorted(views)
hash_sets = {n: frozenset(b["hash"] for b in bl) for n, bl in views.items()}
out["identical_final_views"] = len(set(hash_sets.values())) == 1
out["real_blocks_per_node"] = {n: len(bl) for n, bl in views.items()}

ref = views[min(views)]  # all remaining views are identical (checked above)
by_hash = {b["hash"]: b for b in ref}
by_q = {b["pqp_entry"]["queue_index"]: b for b in ref}
genesis = by_q[0]
real = [b for b in ref if b["pqp_entry"]["queue_index"] != 0]
max_q = max(by_q)
out["tree"] = {"real_blocks_excl_genesis": len(real), "max_slot_index": max_q,
               "max_level": max(b["level"] for b in ref),
               "blocks_per_lane": dict(sorted(collections.Counter(b["align"] for b in real).items()))}
node_of_address = {w["address"]: int(n) for n, w in meta["wallets"].items()}
out["tree"]["blocks_per_miner_node"] = dict(sorted(collections.Counter(
    node_of_address.get(b["pqp_entry"]["miner_address"], "unknown") for b in real).items(), key=str))

# ---- structural checks on the data -------------------------------------------
slot_rule_ok = all(b["pqp_entry"]["queue_index"] == N * by_hash[b["parent_hash"]]["pqp_entry"]["queue_index"] + b["align"]
                   for b in real)
lane_ok = True
for b in real:
    q = b["pqp_entry"]["queue_index"]
    pred = next((by_q[x] for x in range(q - N, 0, -N) if x in by_q), genesis)
    if b["pqp_entry"]["prev_pqp_commitment"] != pred["pqp_commitment"]:
        lane_ok = False
out["checks"] = {"slot_rule_q_eq_N_qparent_plus_lane": slot_rule_ok, "lane_links_point_to_lane_predecessor": lane_ok}
verify = {}
for node in views:
    try:
        verify[node] = {"verify_tree": load(f"final/verify_tree_node{node}.json").get("message"),
                        "verify_pqp": load(f"final/verify_pqp_node{node}.json").get("message")}
    except Exception as err:
        verify[node] = {"error": repr(err)}
out["checks"]["node_self_verification"] = verify

# ---- sibling groups: filled vs empty slots ------------------------------------
pqp = load(f"final/pqp_node{min(views)}.json")["blocks"]
current_parent_q = min(e["queue_index"] for e in pqp)
closed_parents = [b for b in ref if b["pqp_entry"]["queue_index"] < current_parent_q]
children = collections.Counter()
for b in real:
    children[b["parent_hash"]] += 1
hist = collections.Counter(children[p["hash"]] for p in closed_parents)
slots = N * len(closed_parents)
filled = sum(children[p["hash"]] for p in closed_parents)
out["sibling_groups"] = {"closed_parents": len(closed_parents), "current_parent_slot": current_parent_q,
                         "slots_in_closed_groups": slots, "filled": filled, "empty": slots - filled,
                         "fill_rate": round(filled / slots, 4) if slots else None,
                         "children_per_closed_parent": dict(sorted(hist.items()))}

# ---- timing -------------------------------------------------------------------
ts = sorted((b["timestamp"] - t0_ms) / 1000 for b in real)
gaps = [b - a for a, b in zip(ts, ts[1:])]
per_lane_gaps = {}
for lane in range(1, N + 1):
    lts = sorted((b["timestamp"] - t0_ms) / 1000 for b in real if b["align"] == lane)
    lg = [b - a for a, b in zip(lts, lts[1:])]
    per_lane_gaps[lane] = {"blocks": len(lts), "mean_interval_s": round(statistics.mean(lg), 1) if lg else None,
                           "median_interval_s": round(statistics.median(lg), 1) if lg else None}
out["timing"] = {"first_block_s": round(ts[0], 1), "last_block_s": round(ts[-1], 1),
                 "blocks_per_minute": round(len(real) / ((ts[-1]) / 60), 2),
                 "mean_interval_any_lane_s": round(statistics.mean(gaps), 2),
                 "median_interval_any_lane_s": round(statistics.median(gaps), 2),
                 "per_lane": per_lane_gaps,
                 "blocks_per_300s_window": {f"{a}-{a + 300}": sum(1 for x in ts if a <= x < a + 300)
                                            for a in range(0, int(meta["mining_seconds"]), 300)},
                 "bits_values_seen": sorted({b["bits"] for b in ref})}

# ---- difficulty adjustment ------------------------------------------------------
# Offline re-computation of TreeChain::bits_for_upcount (src/treechain/treechain.rs) and
# Block::adjust_bits / target_to_bits (src/treechain/block.rs) from the final tree, then a
# check that every block carries the bits of its retarget period.
def compact_to_target(bits):
    b = bytes.fromhex(bits)
    exponent, mantissa = b[0], int.from_bytes(b[1:4], "big")
    return mantissa << (8 * (exponent - 3)) if exponent > 3 else mantissa >> (8 * (3 - exponent))


def target_to_compact(target):
    raw = target.to_bytes(max(1, (target.bit_length() + 7) // 8), "big").lstrip(b"\0")
    e = len(raw)
    m = int.from_bytes(raw, "big") << (8 * (3 - e)) if e <= 3 else int.from_bytes(raw[:3], "big")
    return f"{(e << 24) | m:08x}"


def adjust_bits(prev_bits, span_ms, expected_ms):
    actual = min(max(span_ms, expected_ms // 4), expected_ms * 4)  # clamp to 1/4x .. 4x
    return target_to_compact(compact_to_target(prev_bits) * actual // expected_ms)


cfg = json.load(open(os.path.join(D, "config_used.json"))) if os.path.exists(os.path.join(D, "config_used.json")) else {}
cfg_rate_ms = cfg.get("mining_rate_ms")
cfg_expected_ms = cfg.get("expected_time_ms")
genesis_target = compact_to_target(genesis["bits"])
slot_order = sorted(ref, key=lambda b: b["pqp_entry"]["queue_index"])  # count c is slot_order[c - 1]; count 1 = genesis
real_slots = [b["pqp_entry"]["queue_index"] for b in slot_order]
difficulty = {}
if cfg_rate_ms and cfg_expected_ms:
    P = (cfg_expected_ms // cfg_rate_ms) * N  # retarget period, in counts (real blocks)

    def upcount(b):
        # get_count_upto_uncle(parent) + align: real blocks before the parent's sibling group, plus the lane
        first_slot_of_group = N * by_hash[b["parent_hash"]]["pqp_entry"]["queue_index"] + 1
        return bisect.bisect_left(real_slots, first_slot_of_group) + b["align"]

    ups = {b["hash"]: upcount(b) for b in real}
    last_period = max(ups.values()) // P
    period_bits, windows = [genesis["bits"]], [None]
    for k in range(1, last_period + 1):
        end = k * P - N
        start = 2 if k == 1 else (k - 1) * P - N  # the first window starts after genesis (timestamp 0)
        bits, win = period_bits[-1], None
        if start < end <= len(slot_order):
            first_ts, last_ts = slot_order[start - 1]["timestamp"], slot_order[end - 1]["timestamp"]
            if last_ts > first_ts:
                span = (last_ts - first_ts) * P // (end - start)  # scaled to P block intervals
                bits = adjust_bits(bits, span, cfg_expected_ms)
                win = {"window_counts": f"{start}-{end}", "window_duration_s": round((last_ts - first_ts) / 1000, 1),
                       "span_scaled_to_P_s": round(span / 1000, 1),
                       "clamped_factor": round(min(max(span / cfg_expected_ms, 0.25), 4.0), 3)}
        period_bits.append(bits)
        windows.append(win)
    mismatches = [{"slot": b["pqp_entry"]["queue_index"], "upcount": ups[b["hash"]], "bits": b["bits"],
                   "expected": period_bits[ups[b["hash"]] // P]}
                  for b in real if b["bits"] != period_bits[ups[b["hash"]] // P]]
    periods = []
    for k in range(last_period + 1):
        members = sorted((b["timestamp"] - t0_ms) / 1000 for b in real if ups[b["hash"]] // P == k)
        n = len(members)
        entry = {"period": k, "upcounts": f"{k * P}-{(k + 1) * P - 1}", "bits": period_bits[k],
                 "difficulty_vs_genesis": round(genesis_target / compact_to_target(period_bits[k]), 3),
                 "blocks": n, "first_block_s": round(members[0], 1) if n else None,
                 "last_block_s": round(members[-1], 1) if n else None,
                 "mean_interval_s": round((members[-1] - members[0]) / (n - 1), 2) if n > 1 else None}
        if windows[k]:
            entry["retarget"] = windows[k]
        periods.append(entry)
    difficulty = {"target_interval_per_lane_s": cfg_rate_ms / 1000,
                  "target_interval_any_lane_s": round(cfg_rate_ms / 1000 / N, 2),
                  "retarget_every_counts": P,
                  "retargets_in_final_tree": sum(1 for k in range(1, last_period + 1) if period_bits[k] != period_bits[k - 1]),
                  "blocks_checked": len(real),
                  "blocks_whose_bits_match_offline_recomputation": len(real) - len(mismatches),
                  "mismatches": mismatches[:10],
                  "periods": periods}
out["difficulty"] = difficulty

# ---- parent age (Observation 1) -----------------------------------------------
pairs = []
for b in real:
    p = by_hash[b["parent_hash"]]
    if p["pqp_entry"]["queue_index"] == 0:
        continue
    tb = (b["timestamp"] - t0_ms) / 1000
    tp = (p["timestamp"] - t0_ms) / 1000
    if tb > 0:
        pairs.append((tp / tb, b["pqp_entry"]["queue_index"]))
ratios = [r for r, _ in pairs]
late = [r for r, q in pairs if q > max_q / 2]
# the same ratio in counts (position in slot order), which does not depend on the block rate
count_of = {b["hash"]: i for i, b in enumerate(sorted(ref, key=lambda b: b["pqp_entry"]["queue_index"]), start=1)}
cpairs = [(count_of[b["parent_hash"]] / count_of[b["hash"]], count_of[b["hash"]]) for b in real
          if by_hash[b["parent_hash"]]["pqp_entry"]["queue_index"] != 0]
cratios = [r for r, _ in cpairs]
clate = [r for r, c in cpairs if c > len(ref) / 2]
fill = out["sibling_groups"]["fill_rate"]
out["parent_age"] = {"median_parent_time_over_block_time": round(statistics.median(ratios), 3) if ratios else None,
                     "median_for_second_half_of_run": round(statistics.median(late), 3) if late else None,
                     "median_parent_count_over_block_count": round(statistics.median(cratios), 3) if cratios else None,
                     "median_count_ratio_second_half": round(statistics.median(clate), 3) if clate else None,
                     "predicted_1_over_fill_rate_times_N": round(1 / (fill * N), 3) if fill else None}

# ---- transactions -------------------------------------------------------------
txlog = [json.loads(line) for line in open(os.path.join(D, "tx_log.jsonl"))]
submitted = len(txlog)
accepted = [t for t in txlog if t["success"]]
err_kinds = collections.Counter()
for t in txlog:
    if not t["success"]:
        e = t["error"] or ""
        key = ("double-spend (wallet's oldest UTXO already pending)" if "Double-spend" in e else
               "timeout" if "Timeout" in e or "timed out" in e else
               "connection refused" if "refused" in e.lower() or "URLError" in e else
               re.sub(r"\d+", "#", e)[:70])  # group messages that differ only in amounts
        err_kinds[key] += 1
included = {}
routing_ok = True
per_lane_tx = collections.Counter()
for b in real:
    for tx in b["tx"][1:]:
        included[tx["txid"]] = (b["timestamp"] - t0_ms) / 1000
        per_lane_tx[b["align"]] += 1
        ss = tx["vin"][0]["script_sig"]
        if lane_of_pubkey_hex(ss) != b["align"]:
            routing_ok = False
lat = [included[t["txid"]] - t["t_s"] for t in accepted if t["txid"] in included]
mempool_left = {}
for node in views:
    try:
        mempool_left[node] = load(f"final/mempool_node{node}.json").get("pool_size")
    except Exception:
        pass
out["transactions"] = {"submission_attempts": submitted, "accepted_into_mempool": len(accepted),
                       "rejections": dict(err_kinds), "confirmed_in_blocks": len(included),
                       "confirmed_per_lane": dict(sorted(per_lane_tx.items())),
                       "every_tx_in_block_of_its_sender_lane": routing_ok,
                       "confirmation_latency_s": {"median": round(statistics.median(lat), 1) if lat else None,
                                                  "mean": round(statistics.mean(lat), 1) if lat else None,
                                                  "p90": round(sorted(lat)[int(0.9 * len(lat)) - 1], 1) if lat else None},
                       "confirmed_tx_per_minute": round(len(included) / (ts[-1] / 60), 2),
                       "mempool_left_at_end": mempool_left}

# ---- log events -----------------------------------------------------------------
patterns = {
    "resync_to_genesis": r"reinitializing|Remote ahead by >24h",
    "slot_already_taken": r"Same Aligned Block already taken",
    "parent_outside_window": r"Pointed to Prev Parent|No current parent and parent_hash mismatch",
    "failed_to_add_block": r"Failed to add (MINED_BLOCK|block)",
    "mining_aborted_new_block": r"aborting current mine",
    "blocks_mined_locally": r"Successfully mined block",
    "pending_block_queued": r"queuing block|Queuing MINED_BLOCK",
    "pending_block_requeued": r"Failed to add pending block",
    "resync_best_peer_longer": r"best peer .* has longer chain",
    "bits_mismatch_rejections": r"Bits mismatch",
    "retargets_computed_by_miner": r"Adjusted bits for queue_index",
    "retargets_verified": r"Verified adjustment for queue_index",
    "coinbase_or_tx_rejections": r"Invalid coinbase value|Transaction validation failed",
}
def read_log(name):
    path = os.path.join(D, "logs", name)
    if os.path.exists(path):
        return open(path, errors="replace").read()
    with gzip.open(path + ".gz", "rt", errors="replace") as fh:  # logs are stored gzipped in the repo
        return fh.read()


events = {}
for node in range(1, 7):
    text = read_log(f"node{node}.log")
    events[node] = {k: len(re.findall(p, text)) for k, p in patterns.items()}
out["log_events_per_node"] = events

# ---- timeline -----------------------------------------------------------------
tl = [json.loads(line) for line in open(os.path.join(D, "timeline.jsonl"))]
snap = collections.defaultdict(dict)
for r in tl:
    snap[r["t_s"]][r["node"]] = r.get("real_blocks")
out["timeline_blocks"] = [{"t_s": t, "blocks": [snap[t].get(n) for n in range(1, 7)]} for t in sorted(snap)]

# ---- liveness (process + HTTP responsiveness at every snapshot) ---------------------
by_t = collections.defaultdict(list)
for r in tl:
    by_t[r["t_s"]].append(r)
alive_known = [r for r in tl if "process_alive" in r]
out["liveness"] = {
    "snapshots": len(by_t),
    "node_snapshots": len(tl),
    "node_snapshots_with_process_alive": sum(1 for r in alive_known if r["process_alive"]),
    "node_snapshots_with_process_state_recorded": len(alive_known),
    "node_snapshots_with_http_error": sum(1 for r in tl if "error" in r),
    "max_get_blocks_ms_per_node": {n: max((r["get_blocks_ms"] for r in tl if r["node"] == n and "get_blocks_ms" in r), default=None)
                                   for n in range(1, 7)},
    "snapshots_with_identical_block_sets": sum(1 for rows in by_t.values()
                                               if len({r.get("block_set_digest") for r in rows}) == 1),
    "max_spread_in_block_count": max(max(r.get("real_blocks") or 0 for r in rows) - min(r.get("real_blocks") or 0 for r in rows)
                                     for rows in by_t.values()),
}
print(json.dumps(out, indent=2))
