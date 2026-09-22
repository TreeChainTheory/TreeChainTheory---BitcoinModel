#!/usr/bin/env python3
"""Download a random sample of real Bitcoin blocks for the lane-routing analysis.

Picks COUNT block heights uniformly at random (fixed seed) from the SPAN blocks that end
at TIP and downloads each raw block from a public Esplora API (blockstream.info and
mempool.space, several downloads in parallel). A block is kept only if its header hashes
to the expected block hash and its transactions hash to the header's Merkle root. Blocks
are stored as raw_blocks/<height>_<hash>.bin; complete blocks already on disk are skipped,
so the script can be re-run after an interruption.

Usage: fetch_blocks.py [out_dir] [--count 200] [--tip 968126] [--span 52560] [--seed 20260922] [--workers 6]
"""
import argparse
import concurrent.futures
import csv
import hashlib
import os
import random
import struct
import subprocess
import sys
import time

from btc_parse import parse_block

APIS = ["https://blockstream.info/api", "https://mempool.space/api"]


def get(path, binary=False, tries=12, first_api=0):
    # curl with a hard time limit per attempt: a stalled connection or a slow DNS lookup
    # can never block a download for more than two minutes
    delay = 2.0
    for attempt in range(tries):
        api = APIS[(first_api + attempt) % len(APIS)]
        result = subprocess.run(["curl", "-sS", "-f", "--connect-timeout", "20", "-m", "120",
                                 "-A", "dettreechain-lane-analysis", api + path], capture_output=True)
        if result.returncode == 0 and result.stdout:
            return result.stdout if binary else result.stdout.decode().strip()
        print(f"  retry {attempt + 1} for {path}: curl exit {result.returncode} "
              f"{result.stderr.decode(errors='replace').strip()[:100]}", flush=True)
        time.sleep(delay)
        delay = min(delay * 2, 60.0)  # DNS hiccups and rate limits usually clear within a minute
    raise RuntimeError(f"giving up on {path}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out_dir", nargs="?", default=os.path.dirname(os.path.abspath(__file__)))
    ap.add_argument("--count", type=int, default=200)
    ap.add_argument("--tip", type=int, default=968126)   # chain tip on 2026-09-22
    ap.add_argument("--span", type=int, default=52560)   # about one year of blocks (365 x 144)
    ap.add_argument("--seed", type=int, default=20260922)
    ap.add_argument("--workers", type=int, default=6)
    args = ap.parse_args()

    raw_dir = os.path.join(args.out_dir, "raw_blocks")
    os.makedirs(raw_dir, exist_ok=True)
    first = args.tip - args.span + 1
    heights = sorted(random.Random(args.seed).sample(range(first, args.tip + 1), args.count))
    print(f"{args.count} heights from {first} to {args.tip} (seed {args.seed})", flush=True)

    def fetch(job):
        i, h = job
        existing = [f for f in os.listdir(raw_dir) if f.startswith(f"{h}_") and f.endswith(".bin")]
        if existing:
            block_hash = existing[0].split("_")[1].split(".")[0]
            raw = open(os.path.join(raw_dir, existing[0]), "rb").read()
            try:
                parse_block(raw)
            except Exception as err:  # an incomplete file from an interrupted run: download again
                print(f"  height {h}: {err}; downloading again", flush=True)
                os.remove(os.path.join(raw_dir, existing[0]))
                existing = []
        if not existing:
            api = 0 if i % 3 else 1  # two thirds of the downloads start on the faster API
            block_hash = get(f"/block-height/{h}", first_api=api)
            raw = get(f"/block/{block_hash}/raw", binary=True, first_api=api)
            parse_block(raw)  # raises if the block is incomplete or corrupt
        header_hash = hashlib.sha256(hashlib.sha256(raw[:80]).digest()).digest()[::-1].hex()
        if header_hash != block_hash:
            raise RuntimeError(f"height {h}: header hashes to {header_hash}, expected {block_hash}")
        if not existing:
            path = os.path.join(raw_dir, f"{h}_{block_hash}.bin")
            with open(path + ".part", "wb") as fh:
                fh.write(raw)
            os.replace(path + ".part", path)  # atomic: a block file on disk is always complete
        timestamp = struct.unpack_from("<I", raw, 68)[0]
        print(f"[{i:3d}/{len(heights)}] height {h} {len(raw) / 1e6:.2f} MB ok", flush=True)
        return {"height": h, "hash": block_hash, "timestamp": timestamp,
                "utc": time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime(timestamp)), "size_bytes": len(raw)}

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
        rows = sorted(pool.map(fetch, enumerate(heights, 1)), key=lambda r: r["height"])

    with open(os.path.join(args.out_dir, "blocks_index.csv"), "w", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    total = sum(r["size_bytes"] for r in rows)
    print(f"done: {len(rows)} blocks, {total / 1e6:.1f} MB in {raw_dir}", flush=True)


if __name__ == "__main__":
    sys.exit(main())
