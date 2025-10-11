import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'

function MiningPage() {
  const [miningStatus, setMiningStatus] = useState('Stopped')
  const [peers, setPeers] = useState([])
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

  useEffect(() => {
    fetchMiningStatus() // Initial fetch
    refreshPeers()
    const interval = setInterval(() => {
      fetchMiningStatus()
      refreshPeers()
    }, 5000) // Poll every 5s
    return () => clearInterval(interval)
  }, [])

  return (
    <div className="pt-20 pb-10 px-4 max-w-4xl mx-auto">
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
      <div className="glass p-6 rounded-xl animate-fade-in">
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
          <ul className="space-y-2">
            {peers.map((peer, i) => (
              <li key={i} className="text-white/90 p-2 bg-white/10 rounded">{peer}</li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}

export default MiningPage