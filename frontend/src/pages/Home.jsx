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
          Welcome to <span className="text-transparent bg-clip-text bg-gradient-to-r from-emerald-400 to-blue-500">TreeChain</span>
        </h1>
        <p className="text-xl text-white/80 mb-8 max-w-3xl mx-auto">
          A revolutionary tree-structured blockchain reimagining consensus, scalability, and security. Dive into the theory behind our innovative approach.
        </p>
        <p className="bg-gradient-to-r from-purple-400 to-blue-500 bg-clip-text text-transparent text-2xl font-semibold">
          Explore the Future of Decentralized Networks
        </p>
      </section>

      {/* Theory Sections */}
      <div className="grid md:grid-cols-2 gap-8 mb-16">
        {/* PQP Section */}
        <article className="glass p-6 rounded-xl shadow-xl animate-fade-in">
          <h2 className="text-3xl font-bold text-white mb-4">Pending Queue of Parents (PQP)</h2>
          <p className="text-white/90 mb-4">
            At the heart of TreeChain is the PQP, a dynamic queue managing parent blocks for branching.
            Each entry includes <code>queue_index</code>, <code>align</code> (1–3 for <code>CHILDREN=3</code>),
            <code>block_hash</code>, <code>parent_hash</code>, <code>miner_address</code>, 
            <code>prev_pqp_commitment</code>, <code>signature</code>, and <code>pqp_commitment</code>.
            This ensures fair mining slots and prevents orphans by aligning children to parents.
          </p>
          <ul className="list-disc list-inside text-white/80 space-y-1">
            <li>Current parent: Lowest <code>queue_index</code> for alignment.</li>
            <li>Next parent: Second lowest to prepare branches.</li>
            <li>Commitments: SHA256 hashes for integrity, signed by miners.</li>
          </ul>
        </article>

        {/* Mining & Consensus Section */}
        <article className="glass p-6 rounded-xl shadow-xl animate-fade-in">
          <h2 className="text-3xl font-bold text-white mb-4">Mining & Consensus</h2>
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
      </div>

      {/* UTXO & Tree Structure Sections */}
      <div className="grid md:grid-cols-2 gap-8 mb-16">
        {/* UTXO Model */}
        <article className="glass p-6 rounded-xl shadow-xl animate-fade-in">
          <h2 className="text-3xl font-bold text-white mb-4">UTXO Model & Transactions</h2>
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

        {/* Tree Structure & Networking */}
        <article className="glass p-6 rounded-xl shadow-xl animate-fade-in">
          <h2 className="text-3xl font-bold text-white mb-4">Tree Structure & P2P Networking</h2>
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
      </div>

      {/* Summary Section */}
      <section className="glass p-8 rounded-xl shadow-xl text-center animate-fade-in">
        <h2 className="text-4xl font-bold text-white mb-6">Our Approach: Scalable, Secure, Fair</h2>
        <p className="text-xl text-white/90 max-w-4xl mx-auto mb-8">
          TreeChain solves the blockchain trilemma by parallelizing via tree — achieving faster finality
          and higher throughput without sharding. PQP democratizes mining slots, while multisig adds
          enterprise-grade security. Built in Rust for performance, Actix for APIs.
        </p>
        <p className="text-xl text-white max-w-4xl mx-auto mb-8">
            You can Scale this by Simply changing the CHILDREN variable to any number in TreeChainTheorey/src/config.rs.
        </p>
        <div className="grid md:grid-cols-3 gap-4 text-white/80">
          <div className="p-4">🚀 <strong>10x Throughput</strong></div>
          <div className="p-4">🔒 <strong>ECDSA Multisig</strong></div>
          <div className="p-4">🌳 <strong>Adaptive Trees </strong></div>
        </div>
      </section>
    </main>
  )
}
