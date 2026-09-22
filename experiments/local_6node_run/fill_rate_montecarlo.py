#!/usr/bin/env python3
"""Monte Carlo of the Parent-Child Completion Constraint alone.

Model: N lanes with equal hash power, zero network delay. Each event is a block
found by a uniformly random lane. A lane mines for the current parent c until it
has a child there, then for the next parent n. A child of n retires c (Rule 2),
as does c receiving its N-th child. Every found block is accepted, so the block
rate is not reduced; the output is the fraction of slots filled in closed
sibling groups.
"""
import random


def simulate(n_lanes, events=300_000, seed=7):
    rng = random.Random(seed)
    filled = [set()]  # filled[i]: lanes with a child under parent i (parents in slot order)
    ci = 0            # current parent; next parent is ci + 1
    closed = []
    for _ in range(events):
        lane = rng.randrange(n_lanes)
        if lane not in filled[ci]:
            target = ci
        elif ci + 1 < len(filled) and lane not in filled[ci + 1]:
            target = ci + 1
        else:
            continue
        filled[target].add(lane)
        filled.append(set())  # the new block is a future parent
        if target == ci + 1 or len(filled[ci]) == n_lanes:
            closed.append(len(filled[ci]))
            ci += 1
    mean_children = sum(closed) / len(closed)
    return mean_children / n_lanes, mean_children


if __name__ == "__main__":
    for n in (1, 2, 3, 4, 8):
        fill, kids = simulate(n)
        print(f"N={n}: fill rate {fill:.3f}, mean children per closed parent {kids:.2f}")


def simulate_with_finder_advantage(n_lanes, miners_per_lane, finder_weight, events=600_000, seed=5):
    """Same rule, but with individual miners and a head start for the miner that found the
    previous block: its rate is multiplied by `finder_weight` for the next block. In the
    prototype, every node that receives a block stops mining while it validates the block and
    rebuilds its template, while the finder continues at once. Returns (fill rate, children
    distribution {1..N: share}, probability that the next block comes from the same miner)."""
    rng = random.Random(seed)
    miners = n_lanes * miners_per_lane
    filled, ci, closed = [set()], 0, []
    last, same, found = None, 0, 0
    for _ in range(events):
        weights = [finder_weight if m == last else 1.0 for m in range(miners)]
        miner = rng.choices(range(miners), weights)[0]
        lane = miner % n_lanes
        if lane not in filled[ci]:
            target = ci
        elif ci + 1 < len(filled) and lane not in filled[ci + 1]:
            target = ci + 1
        else:
            continue
        filled[target].add(lane)
        filled.append(set())
        same += miner == last
        found += 1
        last = miner
        if target == ci + 1 or len(filled[ci]) == n_lanes:
            closed.append(len(filled[ci]))
            ci += 1
    dist = {k: closed.count(k) / len(closed) for k in range(1, n_lanes + 1)}
    return sum(closed) / (n_lanes * len(closed)), dist, same / found
