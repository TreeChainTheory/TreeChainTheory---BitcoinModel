#!/usr/bin/env python3
"""Lane routing on real Bitcoin transactions.

For every non-coinbase transaction in the downloaded blocks, the owner of its first input
is the public key that authorizes the spend (as in the prototype, which routes by the
first input). Two owner functions f are compared for N = 2..8:

  last_hex_digit  lane = (last hex digit of the public key) mod N   (the prototype's rule)
  sha256          lane = (first 64 bits of SHA-256(public key)) mod N

and two scopes of transactions:

  p2pkh       first input is a legacy P2PKH spend (the key ends the unlocking script,
              exactly the case the prototype implements)
  single_key  first input is P2PKH, P2WPKH or P2SH-P2WPKH (key in the script or witness)

Shares are reported per transaction and per distinct key. Writes results.json.

Usage: analyze_lanes.py [dir]
"""
import collections
import csv
import hashlib
import json
import os
import statistics
import sys

from btc_parse import NULL_TXID, classify_input, parse_block

D = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.abspath(__file__))
NS = range(2, 9)
RULES = {
    "last_hex_digit": lambda key: int(key.hex()[-1], 16),
    "sha256": lambda key: int.from_bytes(hashlib.sha256(key).digest()[:8], "big"),
}
SCOPES = {"p2pkh": {"p2pkh"}, "single_key": {"p2pkh", "p2wpkh", "p2sh_p2wpkh"}}

index = list(csv.DictReader(open(os.path.join(D, "blocks_index.csv"))))
kinds = collections.Counter()
tx_keys = {s: [] for s in SCOPES}                      # one owner key per transaction, per scope
per_block = {s: [] for s in SCOPES}                    # per block: list of keys
total_tx = 0
for row in index:
    raw = open(os.path.join(D, "raw_blocks", f"{row['height']}_{row['hash']}.bin"), "rb").read()
    _, txs = parse_block(raw)                          # also checks the Merkle root
    block_keys = {s: [] for s in SCOPES}
    for txid, inputs in txs:
        prev_txid, vout, script_sig, witness = inputs[0]
        if prev_txid == NULL_TXID and vout == 0xFFFFFFFF:
            continue                                   # coinbase
        total_tx += 1
        kind, key = classify_input(script_sig, witness)
        kinds[kind] += 1
        for scope, allowed in SCOPES.items():
            if kind in allowed:
                tx_keys[scope].append(key)
                block_keys[scope].append(key)
    for scope in SCOPES:
        per_block[scope].append(block_keys[scope])


def shares(values, n):
    c = collections.Counter(v % n for v in values)
    total = sum(c.values())
    return [c[i] / total for i in range(n)]


def quantile(sorted_vals, q):
    return sorted_vals[min(len(sorted_vals) - 1, int(q * (len(sorted_vals) - 1) + 0.5))]


out = {"blocks": len(index),
       "height_range": [int(index[0]["height"]), int(index[-1]["height"])],
       "utc_range": [min(r["utc"] for r in index), max(r["utc"] for r in index)],
       "raw_megabytes": round(sum(int(r["size_bytes"]) for r in index) / 1e6, 1),
       "non_coinbase_transactions": total_tx,
       "first_input_kinds": dict(kinds.most_common()),
       "scopes": {}}
for scope in SCOPES:
    keys = tx_keys[scope]
    distinct = list({k: None for k in keys})
    reuse = collections.Counter(keys)
    top = reuse.most_common()
    entry = {"transactions": len(keys), "distinct_keys": len(distinct),
             "share_of_tx_from_top_1pct_keys": round(sum(c for _, c in top[:max(1, len(top) // 100)]) / len(keys), 4),
             "max_tx_per_key": top[0][1],
             "last_hex_digit_histogram": [collections.Counter(k.hex()[-1] for k in distinct)[d] for d in "0123456789abcdef"],
             "rules": {}}
    for rule, f in RULES.items():
        tx_vals = [f(k) for k in keys]
        key_vals = [f(k) for k in distinct]
        entry["rules"][rule] = {}
        for n in NS:
            ts, ks = shares(tx_vals, n), shares(key_vals, n)
            entry["rules"][rule][str(n)] = {
                "tx_shares": [round(x, 5) for x in ts], "key_shares": [round(x, 5) for x in ks],
                "tx_max_dev_pp": round(100 * max(abs(x - 1 / n) for x in ts), 3),
                "key_max_dev_pp": round(100 * max(abs(x - 1 / n) for x in ks), 3)}
    # spread of the lane shares between blocks, N = 4, prototype rule (blocks with >= 20 such transactions)
    per = [shares([RULES["last_hex_digit"](k) for k in bk], 4) for bk in per_block[scope] if len(bk) >= 20]
    entry["per_block_n4"] = {"blocks": len(per),
                             "q10": [round(quantile(sorted(p[l] for p in per), 0.10), 4) for l in range(4)],
                             "median": [round(statistics.median(p[l] for p in per), 4) for l in range(4)],
                             "q90": [round(quantile(sorted(p[l] for p in per), 0.90), 4) for l in range(4)]}
    out["scopes"][scope] = entry

json.dump(out, open(os.path.join(D, "results.json"), "w"), indent=2)
print(json.dumps({k: v for k, v in out.items() if k != "scopes"}, indent=2))
for scope, e in out["scopes"].items():
    print(f"\n[{scope}] {e['transactions']} transactions, {e['distinct_keys']} distinct keys, "
          f"top 1% of keys sign {100 * e['share_of_tx_from_top_1pct_keys']:.1f}% of them")
    for rule in RULES:
        for n in (3, 4):
            r = e["rules"][rule][str(n)]
            print(f"  {rule:14s} N={n}: tx {[round(100 * x, 2) for x in r['tx_shares']]} "
                  f"keys {[round(100 * x, 2) for x in r['key_shares']]}")
