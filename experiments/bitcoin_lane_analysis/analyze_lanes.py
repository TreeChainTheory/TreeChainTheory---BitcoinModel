#!/usr/bin/env python3
"""Lane routing on real Bitcoin transactions under DetTreeChain's routing rule.

Rule (the prototype's rule since 2026-10-04): the lane of an output o is

    lane(o) = (first 64 bits of SHA-256(locking script of o)) mod N + 1,

and a transaction may only spend outputs of one lane.

For every non-coinbase transaction in the downloaded blocks, the locking script of each spent
output is rebuilt from what the spending input reveals (btc_parse.locking_script; checked against
real prevouts from a public API). That is possible for P2PKH, P2WPKH, P2SH-wrapped P2WPKH and
P2WSH, P2SH multisig and native P2WSH inputs. Taproot and a few rare types do not reveal the
locking script when spent, so their transactions are left out; a node, which reads the script from
its UTXO set, has no such limitation.

Measured for N = 2..8:
  * lane shares by transaction (lane of the first input) and by owner (distinct locking script),
  * the spread of the shares between blocks (N = 3),
  * how strongly transactions concentrate on a few owners,
  * for transactions with several inputs, how many already spend outputs of a single lane, i.e.
    are valid under the rule as today's wallets create them.
The prototype's earlier rule (last hex digit of the public key) is evaluated on single-key inputs
for comparison. Writes results.json.

Usage: analyze_lanes.py [dir]
"""
import collections
import csv
import hashlib
import json
import os
import statistics
import sys

from btc_parse import NULL_TXID, classify_input, locking_script, parse_block

D = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.abspath(__file__))
NS = range(2, 9)
BLOCK_SPREAD_N = 3


def script_value(script):
    """First 64 bits of SHA-256(locking script): lane = value mod N + 1."""
    return int.from_bytes(hashlib.sha256(script).digest()[:8], "big")


def shares(values, n):
    c = collections.Counter(v % n for v in values)
    total = sum(c.values())
    return [c[i] / total for i in range(n)]


def max_dev_pp(sh):
    return 100 * max(abs(x - 1 / len(sh)) for x in sh)


def quantile(sorted_vals, q):
    return sorted_vals[min(len(sorted_vals) - 1, int(q * (len(sorted_vals) - 1) + 0.5))]


index = list(csv.DictReader(open(os.path.join(D, "blocks_index.csv"))))
total_tx = 0
total_inputs = revealed_inputs = 0
first_kinds = collections.Counter()
tx_first_values = []                 # one value per transaction whose first input reveals its script
tx_first_scripts = collections.Counter()
per_block_values = []
multi_input = []                     # (values of all inputs, all inputs share one script) when all revealed
multi_input_total = 0
single_input_revealed = 0
key_tx_digits, key_owner_digits = [], {}   # earlier rule on single-key first inputs
for row in index:
    raw = open(os.path.join(D, "raw_blocks", f"{row['height']}_{row['hash']}.bin"), "rb").read()
    _, txs = parse_block(raw)        # also checks the Merkle root
    block_values = []
    for txid, inputs in txs:
        if inputs[0][0] == NULL_TXID and inputs[0][1] == 0xFFFFFFFF:
            continue                 # coinbase
        total_tx += 1
        scripts = []
        for prev_txid, vout, script_sig, witness in inputs:
            kind, script = locking_script(script_sig, witness)
            scripts.append(script)
            total_inputs += 1
            revealed_inputs += script is not None
        first_kind, first_key = classify_input(inputs[0][2], inputs[0][3])
        first_kinds[locking_script(inputs[0][2], inputs[0][3])[0]] += 1
        if scripts[0] is not None:
            v = script_value(scripts[0])
            tx_first_values.append(v)
            tx_first_scripts[scripts[0]] += 1
            block_values.append(v)
        if first_key is not None:
            key_tx_digits.append(int(first_key.hex()[-1], 16))
            key_owner_digits[first_key] = int(first_key.hex()[-1], 16)
        if len(inputs) == 1:
            single_input_revealed += scripts[0] is not None
        else:
            multi_input_total += 1
            if all(s is not None for s in scripts):
                multi_input.append(([script_value(s) for s in scripts], len(set(scripts)) == 1))
    per_block_values.append(block_values)

owner_values = [script_value(s) for s in tx_first_scripts]
top = tx_first_scripts.most_common()
revealed_tx = len(tx_first_values)
out = {
    "rule": "lane = (first 64 bits of SHA-256(locking script)) mod N + 1",
    "blocks": len(index),
    "height_range": [int(index[0]["height"]), int(index[-1]["height"])],
    "utc_range": [min(r["utc"] for r in index), max(r["utc"] for r in index)],
    "raw_megabytes": round(sum(int(r["size_bytes"]) for r in index) / 1e6, 1),
    "non_coinbase_transactions": total_tx,
    "first_input_kinds": dict(first_kinds.most_common()),
    "transactions_first_input_reveals_script": revealed_tx,
    "share_of_transactions_covered": round(revealed_tx / total_tx, 4),
    "inputs_total": total_inputs,
    "inputs_revealing_script": revealed_inputs,
    "owners_distinct_locking_scripts": len(tx_first_scripts),
    "concentration": {
        "share_of_tx_from_top_1pct_owners": round(sum(c for _, c in top[:max(1, len(top) // 100)]) / revealed_tx, 4),
        "max_tx_per_owner": top[0][1],
        "owners_with_one_tx": sum(1 for _, c in top if c == 1),
    },
    "script_hash": {},
    "per_block": {},
    "multi_input": {
        "transactions_with_several_inputs": multi_input_total,
        "with_all_input_scripts_revealed": len(multi_input),
        "share_spending_one_locking_script": round(sum(same for _, same in multi_input) / len(multi_input), 4),
        "by_n": {},
    },
    "earlier_rule_last_hex_digit_of_key": {"transactions": len(key_tx_digits), "keys": len(key_owner_digits), "by_n": {}},
}
for n in NS:
    ts, os_ = shares(tx_first_values, n), shares(owner_values, n)
    out["script_hash"][str(n)] = {"tx_shares": [round(x, 5) for x in ts], "owner_shares": [round(x, 5) for x in os_],
                                  "tx_max_dev_pp": round(max_dev_pp(ts), 3), "owner_max_dev_pp": round(max_dev_pp(os_), 3)}
    same = [all(v % n == vals[0] % n for v in vals) for vals, _ in multi_input]
    # what a wallet that ignores lanes would get if its inputs' scripts were independent
    expected = statistics.mean((1 / n) ** (len(set(vals)) - 1) for vals, _ in multi_input)
    valid_now = (single_input_revealed + sum(same)) / (single_input_revealed + len(multi_input))
    out["multi_input"]["by_n"][str(n)] = {"share_inputs_in_one_lane": round(sum(same) / len(same), 4),
                                          "expected_if_scripts_independent": round(expected, 4),
                                          "share_of_all_transactions_valid_unchanged": round(valid_now, 4)}
    kt, kk = shares(key_tx_digits, n), shares(list(key_owner_digits.values()), n)
    out["earlier_rule_last_hex_digit_of_key"]["by_n"][str(n)] = {
        "tx_shares": [round(x, 5) for x in kt], "key_shares": [round(x, 5) for x in kk],
        "tx_max_dev_pp": round(max_dev_pp(kt), 3), "key_max_dev_pp": round(max_dev_pp(kk), 3)}
per = [shares(v, BLOCK_SPREAD_N) for v in per_block_values if len(v) >= 20]
out["per_block"] = {"n": BLOCK_SPREAD_N, "blocks": len(per),
                    "q10": [round(quantile(sorted(p[l] for p in per), 0.10), 4) for l in range(BLOCK_SPREAD_N)],
                    "median": [round(statistics.median(p[l] for p in per), 4) for l in range(BLOCK_SPREAD_N)],
                    "q90": [round(quantile(sorted(p[l] for p in per), 0.90), 4) for l in range(BLOCK_SPREAD_N)]}

json.dump(out, open(os.path.join(D, "results.json"), "w"), indent=2)
print(json.dumps({k: v for k, v in out.items() if k not in ("script_hash", "multi_input", "earlier_rule_last_hex_digit_of_key")}, indent=2))
for n in (3, 4, 8):
    s, m, e = out["script_hash"][str(n)], out["multi_input"]["by_n"][str(n)], out["earlier_rule_last_hex_digit_of_key"]["by_n"][str(n)]
    print(f"N={n}: tx {[round(100 * x, 1) for x in s['tx_shares']]} owners {[round(100 * x, 1) for x in s['owner_shares']]} "
          f"| multi-input in one lane {100 * m['share_inputs_in_one_lane']:.1f}% (independent {100 * m['expected_if_scripts_independent']:.1f}%), "
          f"all tx valid unchanged {100 * m['share_of_all_transactions_valid_unchanged']:.1f}% | earlier rule keys maxdev {e['key_max_dev_pp']:.2f}pp")
print({k: v for k, v in out["multi_input"].items() if k != "by_n"})
