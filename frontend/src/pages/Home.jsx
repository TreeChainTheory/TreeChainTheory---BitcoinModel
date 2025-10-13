import { useEffect, useState } from 'react'

export default function Home() {
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    setLoaded(true)
  }, [])

  return (
    <main className="pt-20 pb-10 px-4 max-w-7xl mx-auto">
      {/* Hero Section */}
      <section className="text-center mb-16 animate-fade-in">
        <h1 className="text-6xl font-bold text-white mb-4">
          Welcome to <span className="text-transparent bg-clip-text bg-gradient-to-r from-emerald-400 to-blue-500">TreeChainTheorey - Bitcoin model</span>
        </h1>
        <p className="text-xl text-white/80 mb-8 max-w-3xl mx-auto">
          A revolutionary tree-structured blockchain reimagining consensus, scalability, and security. Dive into the theory behind our innovative approach.
        </p>
        <p className="bg-gradient-to-r from-purple-400 to-blue-500 bg-clip-text text-transparent text-2xl font-semibold">
          Explore the Future of Decentralized Networks
        </p>
      </section>

      {/* Tree Structure and PQP Section - Full Width */}
      <article className="glass p-8 rounded-xl shadow-xl animate-fade-in mb-16">
        <h2 className="text-3xl font-bold text-white mb-4">Tree Structure and Parent Queue Pool (PQP)</h2>
        <p className="text-white/90 mb-4">
          TreeChainTheorey replaces the linear reverse-linked list of traditional blockchains with a branching tree structure, where each parent block can produce multiple children (ideal: 2 or 3 per parent), enabling parallel block creation, higher throughput, and better decentralization with multiple active leaders.
        </p>
        <p className="text-white/90 mb-4">
          The Parent Queue Pool (PQP) is a decentralized double-ended queue (deque) that dynamically manages eligible parent blocks for child production, ensuring fair rotation and avoiding bottlenecks. Entries are embedded in blocks for verifiable, consistent updates across nodes.
        </p>
   <ul className="space-y-4 text-white/90 mb-6">
  {[
    {
      key: 'queue_index',
      desc: 'Assigned via breadth-first traversal of the tree — genesis as 0, children numbered left-to-right per level (e.g., 1–3 for first level, 4–12 for second), ensuring ordered, tree-style scheduling in the PQP.'
    },
    {
      key: 'align',
      desc: 'Integer (1 to CHILDREN) indicating the child’s position among its siblings, ensuring balanced branching.'
    },
    {
      key: 'block_hash',
      desc: 'SHA-256 hash of the block being added as a child (hex-encoded).'
    },
    {
      key: 'parent_hash',
      desc: 'Hash linking to its parent block.'
    },
    {
      key: 'miner_address',
      desc: 'Hex-encoded public key or address of the miner creating the block.'
    },
    {
      key: 'prev_pqp_commitment',
      desc: 'Hex-encoded SHA-256 commitment to the previous PQP entry with the same align, ensuring chainable integrity and verifiable updates.'
    },
    {
      key: 'signature',
      desc: 'ECDSA digital signature (hex-encoded) over queue_index + parent_hash + miner_address + prev_pqp_commitment, proving miner authenticity and preventing tampering.'
    },
    {
      key: 'pqp_commitment',
      desc: 'SHA-256 hash of all PQP fields (queue_index + align + block_hash + parent_hash + miner_address + prev_pqp_commitment + signature), serving as a verifiable commitment to the entry’s integrity within the block.'
    }
  ].map((item) => (
    <li key={item.key} className="flex items-start">
      <span className="font-semibold text-white min-w-[160px] md:min-w-[180px]">{item.key}</span>
      <span className="text-white/80">{item.desc}</span>
    </li>
  ))}
</ul>

        <p className="text-white/90 mb-4">
          For transaction assignment: Miners select txns where the last digit of the txn's vin[0].script_sig satisfies (for normal p2pkh txns, and for multisig its vin[0].txid) ((last_digit % CHILDREN as u32) + 1) == align as u32, ensuring balanced distribution across branches.
        </p>
        <p className="text-white/90 mb-4">
          Rollback (for malicious blocks): Prune affected subtrees by removing from the back and re-adding valid parents to the front—<em>TreeChainTheorey - Bitcoin Model does not implement this; reserved for later variants.</em>
        </p>
      </article>

      {/* Mining & Consensus and UTXO Sections */}
      <div className="grid md:grid-cols-2 gap-8 mb-16">
        {/* Mining & Consensus Section */}
        <article className="glass p-6 rounded-xl shadow-xl animate-fade-in">
          <h2 className="text-3xl font-bold text-white mb-4">Mining & Consensus --Bitcoin Model</h2>
          <p className="text-white/90 mb-4">
            Mining occurs every <code>MINING_RATE</code> ms (default 100ms), adjusted by difficulty (<code>BITS</code>)
            every 300 blocks. Subsidy halves every <code>HALVING_INTERVAL</code> (100 blocks). Tree branching:
            Each parent spawns up to <code>CHILDREN</code> (3) children via align. Queue index calculated as
            <code>max(parent QI) + align offset</code>.
          </p>
          <ul className="list-disc list-inside text-white/80 space-y-1">
            <li>PoW: Nonce iteration until <code>hash &lt; target</code> (BigUint-based).</li>
            <li>Difficulty: Adjusted based on timestamps (clamped 1/4×–4×).</li>
            <li>Block Template: Includes coinbase (subsidy + fees), Merkle root, PQP entry signed by miner.</li>
          </ul>
        </article>

        {/* UTXO Model Section */}
        <article className="glass p-6 rounded-xl shadow-xl animate-fade-in">
          <h2 className="text-3xl font-bold text-white mb-4">UTXO Model & Transactions --Bitcoin Model</h2>
          <p className="text-white/90 mb-4">
            Bitcoin-inspired UTXO set tracks unspent outputs with <code>queue_index</code> for maturity.
            Supports P2PKH, P2SH multisig (m-of-n redeem scripts), and SegWit witnesses. Mempool validates
            fees (min sat/vB), dependencies, and RBF (BIP-125). Transaction creation involves input selection,
            sighash (<code>SIGHASH_ALL</code>), and ECDSA signing (<code>k256</code>).
          </p>
          <ul className="list-disc list-inside text-white/80 space-y-1">
            <li>Multisig: Create redeem script, P2SH hash, spend with m sigs appended to script_sig.</li>
            <li>Validation: Locktime, sig verification, and no double-spends.</li>
            <li>Balance: Sums mature UTXOs (coinbase after 100 confirmations).</li>
          </ul>
        </article>
      </div>

      {/* P2P Networking Section - Full Width */}
      <article className="glass p-8 rounded-xl shadow-xl animate-fade-in mb-16">
        <h2 className="text-3xl font-bold text-white mb-4">P2P Networking</h2>
        <p className="text-white/90 mb-4">
          Blocks form a tree via <code>parent_hash</code> and <code>children_map</code>. Levels/positions track depth
          (e.g., "0.1.2"). P2P (Tokio TCP) handles messages like <code>MINED_BLOCK</code>, <code>INV_MESSAGE</code>,
          and <code>TRANSACTION</code>. Sync via IBD (<code>getblocks</code> / inventories), with a registry for peer
          discovery. Reorgs safely invalidate outdated transactions.
        </p>
        <ul className="list-disc list-inside text-white/80 space-y-1">
          <li>Alignment: Ensures balanced branching (no &gt; CHILDREN per parent).</li>
          <li>Broadcast: New blocks and transactions flood to peers.</li>
          <li>Verification: Hash/PQP commitment, parent existence, and tx validity.</li>
        </ul>
      </article>

      {/* Summary Section */}
      <section className="glass p-8 rounded-xl shadow-xl text-center animate-fade-in">
        <h2 className="text-4xl font-bold text-white mb-6">Our Approach: Scalable, Secure, Fair</h2>
        <p className="text-xl text-white/90 max-w-4xl mx-auto mb-8">
          TreeChainTheorey solves the blockchain trilemma by parallelizing via tree — achieving faster finality
          and higher throughput without sharding. PQP democratizes mining slots, while multisig adds
          enterprise-grade security. Built in Rust for performance, Actix for APIs.
        </p>
        <div className="text-white/80 mb-4">
          <p><strong>Note 1:</strong> Everything in here is all hex's unlike the real ones, as this is all just a prototype and need for human readability to verify.</p>
          <p><strong>Note 2:</strong> You can Scale this by Simply changing the CHILDREN variable to any number in TreeChainTheorey/src/config.rs.</p>
        </div>
        <div className="grid md:grid-cols-3 gap-4 text-white/80">
          <div className="p-4">🚀 <strong>10x Throughput</strong></div>
          <div className="p-4">🔒 <strong>ECDSA Multisig</strong></div>
          <div className="p-4">🌳 <strong>Adaptive Trees </strong></div>
        </div>
      </section>
    </main>
  )
}