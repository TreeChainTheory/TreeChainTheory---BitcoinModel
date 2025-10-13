import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'

function MiningPage() {
  const [miningStatus, setMiningStatus] = useState('Stopped')
  const [peers, setPeers] = useState([])
  const [txPool, setTxPool] = useState([])
  const [poolSize, setPoolSize] = useState(0)
  const [avgFeeRate, setAvgFeeRate] = useState(0)
  const [totalFeesSats, setTotalFeesSats] = useState(0)
  const [totalSizeBytes, setTotalSizeBytes] = useState(0)
  const [totalValueSats, setTotalValueSats] = useState(0)
  const [txTimestamp, setTxTimestamp] = useState('')
  const [utxoSet, setUtxoSet] = useState([])
  const [totalUtxoValue, setTotalUtxoValue] = useState(0)
  const [utxoTimestamp, setUtxoTimestamp] = useState('')
  const [loading, setLoading] = useState(false)
  const navigate = useNavigate()
  const apiBase = import.meta.env.VITE_API_BASE
  console.log('API Base URL:', apiBase) // Debugging line

  const fetchMiningStatus = async () => {
    try {
      const res = await fetch(`${apiBase}/get_mining_status`)
      const data = await res.json()
      if (data.status === 'success') {
        setMiningStatus(data.mining ? 'Started' : 'Stopped')
      }
    } catch (err) {
      console.error('Error fetching mining status:', err)
    }
  }

  const startMining = async () => {
    setLoading(true)
    try {
      await fetch(`${apiBase}/start_mining`)
      setMiningStatus('Started')
    } catch (err) {
      alert('Error starting mining')
    }
    setLoading(false)
  }

  const stopMining = async () => {
    setLoading(true)
    try {
      await fetch(`${apiBase}/stop_mining`)
      setMiningStatus('Stopped')
    } catch (err) {
      alert('Error stopping mining')
    }
    setLoading(false)
  }

  const refreshPeers = async () => {
    setLoading(true)
    try {
      const res = await fetch(`${apiBase}/get_connected_peers`)
      const data = await res.json()
      setPeers(data.peers || [])
    } catch (err) {
      alert('Error fetching peers')
    }
    setLoading(false)
  }

  const fetchTransactionPool = async () => {
    try {
      const res = await fetch(`${apiBase}/transaction_pool`)
      const data = await res.json()
      if (data.success) {
        setTxPool(data.transactions || [])
        setPoolSize(data.pool_size || 0)
        setAvgFeeRate(data.average_fee_rate_sat_per_vb || 0)
        setTotalFeesSats(data.total_fees_satoshis || 0)
        setTotalSizeBytes(data.total_size_bytes || 0)
        setTotalValueSats(data.total_value_satoshis || 0)
        setTxTimestamp(data.timestamp || '')
      }
    } catch (err) {
      console.error('Error fetching transaction pool:', err)
    }
  }

  const fetchUtxoSet = async () => {
    try {
      const res = await fetch(`${apiBase}/utxo_set`)
      const data = await res.json()
      if (data.success) {
        setUtxoSet(data.utxos || [])
        setTotalUtxoValue(data.total_value || 0)
        setUtxoTimestamp(data.timestamp || '')
      }
    } catch (err) {
      console.error('Error fetching UTXO set:', err)
    }
  }

  const formatAge = (seconds) => {
    if (seconds < 60) return `${seconds}s ago`
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
    return `${Math.floor(seconds / 3600)}h ago`
  }

  const satsToBtc = (sats) => (sats / 100000000).toFixed(8)

  useEffect(() => {
    fetchMiningStatus() // Initial fetch
    refreshPeers()
    fetchTransactionPool()
    fetchUtxoSet()
    const interval = setInterval(() => {
      fetchMiningStatus()
      refreshPeers()
      fetchTransactionPool()
      fetchUtxoSet()
    }, 5000) // Poll every 5s
    return () => clearInterval(interval)
  }, [])

  return (
    <div className="pt-20 pb-10 px-4 max-w-6xl mx-auto">
      <h1 className="text-4xl font-bold text-white mb-8 text-center animate-fade-in">Mining Dashboard</h1>
      
      {/* Mining Controls */}
      <div className="glass p-6 rounded-xl mb-8 text-center animate-fade-in">
        <h2 className="text-2xl text-white mb-4">Status: {miningStatus}</h2>
        <div className="space-x-4">
          <button
            onClick={startMining}
            disabled={miningStatus === 'Started' || loading}
            className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600 transition-all disabled:opacity-50"
          >
            Start Mining
          </button>
          <button
            onClick={stopMining}
            disabled={miningStatus === 'Stopped' || loading}
            className="px-6 py-3 bg-red-500 text-white rounded-lg hover:bg-red-600 transition-all disabled:opacity-50"
          >
            Stop Mining
          </button>
        </div>
      </div>

      {/* Peers List */}
      <div className="glass p-6 rounded-xl mb-8 animate-fade-in">
        <div className="flex justify-between items-center mb-4">
          <h2 className="text-2xl text-white">Connected Peers ({peers.length})</h2>
          <button
            onClick={refreshPeers}
            disabled={loading}
            className="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all disabled:opacity-50"
          >
            Refresh
          </button>
        </div>
        {peers.length === 0 ? (
          <p className="text-white/80 text-center">No peers connected.</p>
        ) : (
          <ul className="space-y-2 max-h-64 overflow-y-auto">
            {peers.map((peer, i) => (
              <li key={i} className="text-white/90 p-2 bg-white/10 rounded">{peer}</li>
            ))}
          </ul>
        )}
      </div>

      {/* Transaction Pool */}
      <div className="glass p-6 rounded-xl mb-8 animate-fade-in">
        <div className="flex justify-between items-center mb-4">
          <div>
            <h2 className="text-2xl text-white">Transaction Pool ({poolSize})</h2>
            {txTimestamp && <p className="text-white/70 text-sm">Updated: {new Date(txTimestamp).toLocaleString()}</p>}
            <div className="text-white/70 text-sm mt-1 space-y-1">
              <p>Avg Fee Rate: {avgFeeRate.toFixed(2)} sat/vB</p>
              <p>Total Fees: {totalFeesSats.toLocaleString()} sats ({satsToBtc(totalFeesSats)} BTC)</p>
              <p>Total Size: {totalSizeBytes} bytes</p>
              <p>Total Value: {totalValueSats.toLocaleString()} sats</p>
            </div>
          </div>
          <button
            onClick={fetchTransactionPool}
            disabled={loading}
            className="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all disabled:opacity-50"
          >
            Refresh
          </button>
        </div>
        {poolSize === 0 ? (
          <p className="text-white/80 text-center">Transaction pool is empty.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-white/90 text-xs border-collapse">
              <thead>
                <tr className="bg-white/10">
                  <th className="p-2 text-left">TXID</th>
                  <th className="p-2 text-left">Fee (sats)</th>
                  <th className="p-2 text-left">Fee Rate (sat/vB)</th>
                  <th className="p-2 text-left">Size (vB)</th>
                  <th className="p-2 text-left">Age</th>
                  <th className="p-2 text-left">Inputs/Outputs</th>
                </tr>
              </thead>
              <tbody className="max-h-64 overflow-y-auto">
                {txPool.map((tx, i) => (
                  <tr key={i} className="border-b border-white/10 hover:bg-white/5">
                    <td className="p-2 font-mono truncate max-w-32">{tx.txid.substring(0, 16)}...</td>
                    <td className="p-2">{tx.fee?.satoshis?.toLocaleString() || 'N/A'}</td>
                    <td className="p-2">{tx.fee_rate?.sat_per_vbyte?.toFixed(2) || 'N/A'}</td>
                    <td className="p-2">{tx.size?.vbytes || 'N/A'}</td>
                    <td className="p-2">{formatAge(tx.age_seconds?.value || 0)}</td>
                    <td className="p-2">{tx.input_count || 0}/{tx.output_count || 0}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* UTXO Set */}
      <div className="glass p-6 rounded-xl animate-fade-in">
        <div className="flex justify-between items-center mb-4">
          <div>
            <h2 className="text-2xl text-white">UTXO Set ({utxoSet.length})</h2>
            {utxoTimestamp && <p className="text-white/70 text-sm">Updated: {new Date(utxoTimestamp).toLocaleString()}</p>}
            {totalUtxoValue > 0 && <p className="text-white/70 text-sm">Total Value: {totalUtxoValue.toLocaleString()} sats</p>}
          </div>
          <button
            onClick={fetchUtxoSet}
            disabled={loading}
            className="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all disabled:opacity-50"
          >
            Refresh
          </button>
        </div>
        {utxoSet.length === 0 ? (
          <p className="text-white/80 text-center">UTXO set is empty.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-white/90 text-xs border-collapse">
              <thead>
                <tr className="bg-white/10">
                  <th className="p-2 text-left">TXID</th>
                  <th className="p-2 text-left">Vout</th>
                  <th className="p-2 text-left">Value (sats)</th>
                  <th className="p-2 text-left">Queue Index</th>
                  <th className="p-2 text-left">Coinbase</th>
                  <th className="p-2 text-left">Script Pubkey</th>
                </tr>
              </thead>
              <tbody className="max-h-64 overflow-y-auto">
                {utxoSet.map((utxo, i) => (
                  <tr key={i} className="border-b border-white/10 hover:bg-white/5">
                    <td className="p-2 font-mono truncate max-w-32">{utxo.txid.substring(0, 16)}...</td>
                    <td className="p-2">{utxo.vout}</td>
                    <td className="p-2">{utxo.value.toLocaleString()}</td>
                    <td className="p-2">QI: {utxo.queue_index}</td>
                    <td className="p-2">{utxo.is_coinbase ? 'Yes' : 'No'}</td>
                    <td className="p-2 font-mono truncate max-w-40">{utxo.script_pubkey.substring(0, 20)}...</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  )
}

export default MiningPage