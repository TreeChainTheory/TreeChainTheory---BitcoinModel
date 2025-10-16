import { useState, useEffect } from 'react'

function WalletPage() {
  const [wallet, setWallet] = useState({
    address: '',
    public_key: '',
    balance_sat: 0,
    balance_btc: '0.00000000'
  })
  const [loading, setLoading] = useState(true)
  const [activeTab, setActiveTab] = useState('balance')
  const [submitting, setSubmitting] = useState(false)
  const apiBase = import.meta.env.VITE_API_BASE

  // Separate state for each tab
  const [normalTxnState, setNormalTxnState] = useState({
    toAddress: '',
    value_btc: '',
    fee_btc: '',
    result: null
  })

  const [multisigCreateState, setMultisigCreateState] = useState({
    pubkeys: '',
    m: 2,
    value_btc: '',
    fee_btc: '',
    result: null
  })

  const [findMultisigState, setFindMultisigState] = useState({
    pubkeys: '',
    m: 2,
    result: null
  })

  const [spendingCreateState, setSpendingCreateState] = useState({
    txid: '',
    vout: 0,
    pubkeys: '',
    m: 2,
    toAddress: '',
    value_btc: '',
    fee_btc: '',
    result: null
  })

  const [signState, setSignState] = useState({
    txid: '',
    vout: 0,
    spendingTxJson: '',
    pubkeys: '',
    m: 2,
    result: null
  })

  const [spendState, setSpendState] = useState({
    spendingTxJson: '',
    pubkeys: '',
    m: 2,
    sigs: '',
    result: null
  })

  const [checkBalanceState, setCheckBalanceState] = useState({
    address: '',
    result: null
  })

  useEffect(() => {
    fetchWallet()
  }, [])

  const fetchWallet = async () => {
    try {
      const res = await fetch(`${apiBase}/wallet`)
      const data = await res.json()
      const balance_sat = data.wallet.balance_satoshis.total
      setWallet({
        address: data.wallet.address,
        public_key: data.wallet.public_key,
        balance_sat: balance_sat,
        balance_btc: data.wallet.balance_btc
      })
      showToast('Wallet loaded successfully!', 'success')
    } catch (err) {
      console.error(err)
      showToast('Failed to load wallet', 'error')
    }
    setLoading(false)
  }

  const showToast = (message, type) => {
    // Simple toast implementation
    const toast = document.createElement('div')
    toast.className = `fixed top-4 right-4 px-6 py-3 rounded-lg text-white z-50 ${
      type === 'success' ? 'bg-green-500' : 'bg-red-500'
    }`
    toast.textContent = message
    document.body.appendChild(toast)
    setTimeout(() => toast.remove(), 3000)
  }

  const btcToSat = (btc) => {
    const value = parseFloat(btc)
    if (isNaN(value)) return 0
    return Math.floor(value * 100000000)
  }

  const copyJson = (json) => {
    navigator.clipboard.writeText(JSON.stringify(json, null, 2))
    showToast('Copied to clipboard!', 'success')
  }

  // Normal Transaction Handlers
  const handleNormalTxn = async () => {
    if (!normalTxnState.toAddress || !normalTxnState.value_btc || !normalTxnState.fee_btc) {
      showToast('Please fill all fields', 'error')
      return
    }

    const value = parseFloat(normalTxnState.value_btc)
    const fee = parseFloat(normalTxnState.fee_btc)

    if (isNaN(value) || isNaN(fee) || value <= 0 || fee <= 0) {
      showToast('Please enter valid BTC amounts', 'error')
      return
    }

    setSubmitting(true)
    try {
      const body = {
        to_address: normalTxnState.toAddress,
        value: btcToSat(normalTxnState.value_btc),
        fee: btcToSat(normalTxnState.fee_btc)
      }

      const res = await fetch(`${apiBase}/create_txn`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body)
      })
      const data = await res.json()

      if (!data.success) {
        showToast(data.error || 'Operation failed', 'error')
        return
      }

      setNormalTxnState(prev => ({ ...prev, result: data }))
      showToast('Transaction created successfully', 'success')
    } catch (err) {
      console.error(err)
      showToast(`Error: ${err.message}`, 'error')
    } finally {
      setSubmitting(false)
    }
  }

  const resetNormalTxn = () => {
    setNormalTxnState({
      toAddress: '',
      value_btc: '',
      fee_btc: '',
      result: null
    })
  }

  // Multisig Create Handlers
  const handleMultisigCreate = async () => {
    if (!multisigCreateState.pubkeys || !multisigCreateState.value_btc || !multisigCreateState.fee_btc) {
      showToast('Please fill all fields', 'error')
      return
    }

    const value = parseFloat(multisigCreateState.value_btc)
    const fee = parseFloat(multisigCreateState.fee_btc)

    if (isNaN(value) || isNaN(fee) || value <= 0 || fee <= 0) {
      showToast('Please enter valid BTC amounts', 'error')
      return
    }

    const pubkeys = multisigCreateState.pubkeys.split(',').map(p => p.trim()).filter(p => p.length > 0)
    if (pubkeys.length < multisigCreateState.m) {
      showToast(`Need at least ${multisigCreateState.m} public keys`, 'error')
      return
    }

    setSubmitting(true)
    try {
      const body = {
        pubkeys,
        m: multisigCreateState.m,
        value: btcToSat(multisigCreateState.value_btc),
        fee: btcToSat(multisigCreateState.fee_btc)
      }

      const res = await fetch(`${apiBase}/create_multisig_txn`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body)
      })
      const data = await res.json()

      if (data.status !== 'success') {
        showToast(data.message || 'Operation failed', 'error')
        return
      }

      setMultisigCreateState(prev => ({ ...prev, result: data }))
      showToast('Multisig transaction created successfully', 'success')
    } catch (err) {
      console.error(err)
      showToast(`Error: ${err.message}`, 'error')
    } finally {
      setSubmitting(false)
    }
  }

  const resetMultisigCreate = () => {
    setMultisigCreateState({
      pubkeys: '',
      m: 2,
      value_btc: '',
      fee_btc: '',
      result: null
    })
  }

  // Find Multisig Handlers
  const handleFindMultisig = async () => {
    if (!findMultisigState.pubkeys) {
      showToast('Please enter public keys', 'error')
      return
    }

    const pubkeys = findMultisigState.pubkeys.split(',').map(p => p.trim()).filter(p => p.length > 0)
    if (pubkeys.length < findMultisigState.m) {
      showToast(`Need at least ${findMultisigState.m} public keys`, 'error')
      return
    }

    setSubmitting(true)
    try {
      const body = {
        pubkeys,
        m: findMultisigState.m
      }

      const res = await fetch(`${apiBase}/find_multisig_utxo`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body)
      })
      const data = await res.json()

      if (data.status !== 'success') {
        showToast(data.message || 'Operation failed', 'error')
        return
      }

      setFindMultisigState(prev => ({ ...prev, result: data }))
      showToast('Multisig UTXOs fetched successfully', 'success')
    } catch (err) {
      console.error(err)
      showToast(`Error: ${err.message}`, 'error')
    } finally {
      setSubmitting(false)
    }
  }

  const resetFindMultisig = () => {
    setFindMultisigState({
      pubkeys: '',
      m: 2,
      result: null
    })
  }

  // Spending Create Handlers
  const handleCreateSpending = async () => {
    if (!spendingCreateState.txid || !spendingCreateState.pubkeys ||
        !spendingCreateState.toAddress || !spendingCreateState.value_btc ||
        !spendingCreateState.fee_btc) {
      showToast('Please fill all fields', 'error')
      return
    }

    const value = parseFloat(spendingCreateState.value_btc)
    const fee = parseFloat(spendingCreateState.fee_btc)

    if (isNaN(value) || isNaN(fee) || value <= 0 || fee <= 0) {
      showToast('Please enter valid BTC amounts', 'error')
      return
    }

    const pubkeys = spendingCreateState.pubkeys.split(',').map(p => p.trim()).filter(p => p.length > 0)
    if (pubkeys.length < spendingCreateState.m) {
      showToast(`Need at least ${spendingCreateState.m} public keys`, 'error')
      return
    }

    setSubmitting(true)
    try {
      const body = {
        txid: spendingCreateState.txid,
        vout: spendingCreateState.vout,
        pubkeys,
        m: spendingCreateState.m,
        to_address: spendingCreateState.toAddress,
        value: btcToSat(spendingCreateState.value_btc),
        fee: btcToSat(spendingCreateState.fee_btc)
      }

      const res = await fetch(`${apiBase}/create_spending_multisig_tx`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body)
      })
      const data = await res.json()

      if (data.status !== 'success') {
        showToast(data.message || 'Operation failed', 'error')
        return
      }

      setSpendingCreateState(prev => ({ ...prev, result: data }))
      showToast('Unsigned spending transaction created', 'success')
    } catch (err) {
      console.error(err)
      showToast(`Error: ${err.message}`, 'error')
    } finally {
      setSubmitting(false)
    }
  }

  const resetSpendingCreate = () => {
    setSpendingCreateState({
      txid: '',
      vout: 0,
      pubkeys: '',
      m: 2,
      toAddress: '',
      value_btc: '',
      fee_btc: '',
      result: null
    })
  }

  // Sign Handlers
  const handleSignMultisig = async () => {
    if (!signState.txid || !signState.spendingTxJson || !signState.pubkeys) {
      showToast('Please fill all fields', 'error')
      return
    }

    let spendingTx
    try {
      spendingTx = JSON.parse(signState.spendingTxJson)
    } catch (e) {
      showToast('Invalid JSON for spending transaction', 'error')
      return
    }

    const pubkeys = signState.pubkeys.split(',').map(p => p.trim()).filter(p => p.length > 0)
    if (pubkeys.length < signState.m) {
      showToast(`Need at least ${signState.m} public keys`, 'error')
      return
    }

    setSubmitting(true)
    try {
      const body = {
        txid: signState.txid,
        vout: signState.vout,
        spending_tx: spendingTx,
        pubkeys,
        m: signState.m
      }

      const res = await fetch(`${apiBase}/sign_multisig`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body)
      })
      const data = await res.json()

      if (data.status !== 'success') {
        showToast(data.message || 'Operation failed', 'error')
        return
      }

      setSignState(prev => ({ ...prev, result: data }))
      showToast('Transaction signed successfully', 'success')
    } catch (err) {
      console.error(err)
      showToast(`Error: ${err.message}`, 'error')
    } finally {
      setSubmitting(false)
    }
  }

  const resetSign = () => {
    setSignState({
      txid: '',
      vout: 0,
      spendingTxJson: '',
      pubkeys: '',
      m: 2,
      result: null
    })
  }

  // Spend Handlers
  const handleSpendMultisig = async () => {
    if (!spendState.spendingTxJson || !spendState.pubkeys || !spendState.sigs) {
      showToast('Please fill all fields', 'error')
      return
    }

    let spendingTx
    try {
      spendingTx = JSON.parse(spendState.spendingTxJson)
    } catch (e) {
      showToast('Invalid JSON for spending transaction', 'error')
      return
    }

    const pubkeys = spendState.pubkeys.split(',').map(p => p.trim()).filter(p => p.length > 0)
    const sigs = spendState.sigs.split(',').map(s => s.trim()).filter(s => s.length > 0)

    if (pubkeys.length < spendState.m) {
      showToast(`Need at least ${spendState.m} public keys`, 'error')
      return
    }

    if (sigs.length < spendState.m) {
      showToast(`Need at least ${spendState.m} signatures`, 'error')
      return
    }

    setSubmitting(true)
    try {
      const body = {
        spending_tx: spendingTx,
        pubkeys,
        m: spendState.m,
        sigs
      }

      const res = await fetch(`${apiBase}/spend_multisig_txn`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body)
      })
      const data = await res.json()
      console.log("data: ",data);
      if (!data.success) {
        showToast(data.message || 'Operation failed', 'error')
        return
      }

      setSpendState(prev => ({ ...prev, result: data }))
      showToast('Multisig transaction spent successfully', 'success')
    } catch (err) {
      console.error(err)
      showToast(`Error: ${err.message}`, 'error')
    } finally {
      setSubmitting(false)
    }
  }

  const resetSpend = () => {
    setSpendState({
      spendingTxJson: '',
      pubkeys: '',
      m: 2,
      sigs: '',
      result: null
    })
  }

  // Check Balance Handlers
  const handleCheckBalance = async () => {
    if (!checkBalanceState.address) {
      showToast('Please enter an address', 'error')
      return
    }

    setSubmitting(true)
    try {
      const res = await fetch(`${apiBase}/balance/${checkBalanceState.address}`)
      const data = await res.json()

      if (data.status !== 'success') {
        showToast(data.message || 'Error fetching balance', 'error')
        return
      }

      setCheckBalanceState(prev => ({ ...prev, result: data.balance }))
      showToast('Balance fetched successfully', 'success')
    } catch (err) {
      console.error(err)
      showToast(`Error: ${err.message}`, 'error')
    } finally {
      setSubmitting(false)
    }
  }

  const resetCheckBalance = () => {
    setCheckBalanceState({
      address: '',
      result: null
    })
  }

  if (loading) {
    return (
      <div className="flex justify-center items-center h-screen bg-gradient-to-br from-slate-900 via-blue-900 to-slate-900">
        <div className="animate-spin rounded-full h-32 w-32 border-b-2 border-white"></div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-slate-900 via-blue-900 to-slate-900 pt-20 pb-10 px-4">
      <div className="max-w-6xl mx-auto">
        <h1 className="text-4xl font-bold text-white mb-8 text-center">Wallet Dashboard</h1>

        {/* Tabs */}
        <div className="bg-white/10 backdrop-blur-md p-4 rounded-xl mb-8 flex flex-wrap gap-2">
          {[
            { key: 'balance', label: 'Balance' },
            { key: 'normal', label: 'Normal Txn' },
            { key: 'multisig', label: 'Create Multisig' },
            { key: 'find', label: 'Find Multisig' },
            { key: 'spending', label: 'Create Spending' },
            { key: 'sign', label: 'Sign' },
            { key: 'spend', label: 'Spend' },
            { key: 'check', label: 'Check Balance' }
          ].map(tab => (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              className={`px-4 py-2 rounded-lg transition-all ${
                activeTab === tab.key
                  ? 'bg-white/20 text-white shadow-lg'
                  : 'text-white/70 hover:bg-white/10'
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* Balance Tab */}
        {activeTab === 'balance' && (
          <div className="bg-white/10 backdrop-blur-md p-6 rounded-xl">
            <h2 className="text-2xl text-white mb-4">Balance</h2>
            <div className="space-y-3">
              <div>
                <p className="text-white/70 text-sm mb-1">Address</p>
                <p className="font-mono bg-white/10 px-3 py-2 rounded text-white break-all">
                  {wallet.address}
                </p>
              </div>
              <div>
                <p className="text-white/70 text-sm mb-1">Public Key</p>
                <p className="font-mono bg-white/10 px-3 py-2 rounded text-white break-all">
                  {wallet.public_key}
                </p>
              </div>
              <div>
                <p className="text-white/70 text-sm mb-1">Balance</p>
                <p className="text-3xl text-green-300 font-bold">{wallet.balance_btc} BTC</p>
                <p className="text-white/70 text-sm">{wallet.balance_sat} satoshis</p>
              </div>
            </div>
            <button
              onClick={fetchWallet}
              disabled={submitting}
              className="mt-6 px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50 transition-all"
            >
              Refresh Balance
            </button>
          </div>
        )}

        {/* Normal Transaction Tab */}
        {activeTab === 'normal' && (
          <div className="bg-white/10 backdrop-blur-md p-6 rounded-xl">
            <h2 className="text-2xl text-white mb-4">Create Normal Transaction</h2>
            {!normalTxnState.result ? (
              <>
                <div className="space-y-4 mb-6">
                  <input
                    placeholder="To Address"
                    value={normalTxnState.toAddress}
                    onChange={(e) => setNormalTxnState(prev => ({ ...prev, toAddress: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <input
                    placeholder="Value (BTC)"
                    value={normalTxnState.value_btc}
                    onChange={(e) => setNormalTxnState(prev => ({ ...prev, value_btc: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <input
                    placeholder="Fee (BTC)"
                    value={normalTxnState.fee_btc}
                    onChange={(e) => setNormalTxnState(prev => ({ ...prev, fee_btc: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                </div>
                <button
                  onClick={handleNormalTxn}
                  disabled={submitting}
                  className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:opacity-50 transition-all"
                >
                  {submitting ? 'Sending...' : 'Send Transaction'}
                </button>
              </>
            ) : (
              <>
                <div className="mb-6">
                  <h3 className="text-white text-lg mb-3">Transaction Result</h3>
                  <pre className="p-4 bg-white/10 rounded-lg text-white/90 overflow-auto text-sm">
                    {JSON.stringify(normalTxnState.result, null, 2)}
                  </pre>
                </div>
                <button
                  onClick={resetNormalTxn}
                  className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all"
                >
                  Create Another Transaction
                </button>
              </>
            )}
          </div>
        )}

        {/* Create Multisig Tab */}
        {activeTab === 'multisig' && (
          <div className="bg-white/10 backdrop-blur-md p-6 rounded-xl">
            <h2 className="text-2xl text-white mb-4">Create Multisig Transaction</h2>
            {!multisigCreateState.result ? (
              <>
                <div className="space-y-4 mb-6">
                  <textarea
                    placeholder="Public Keys (comma-separated)"
                    value={multisigCreateState.pubkeys}
                    onChange={(e) => setMultisigCreateState(prev => ({ ...prev, pubkeys: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none h-24"
                  />
                  <input
                    type="number"
                    placeholder="Required Signatures (m)"
                    value={multisigCreateState.m}
                    onChange={(e) => setMultisigCreateState(prev => ({ ...prev, m: parseInt(e.target.value) || 2 }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <input
                    placeholder="Value (BTC)"
                    value={multisigCreateState.value_btc}
                    onChange={(e) => setMultisigCreateState(prev => ({ ...prev, value_btc: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <input
                    placeholder="Fee (BTC)"
                    value={multisigCreateState.fee_btc}
                    onChange={(e) => setMultisigCreateState(prev => ({ ...prev, fee_btc: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                </div>
                <button
                  onClick={handleMultisigCreate}
                  disabled={submitting}
                  className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:opacity-50 transition-all"
                >
                  {submitting ? 'Creating...' : 'Create Multisig'}
                </button>
              </>
            ) : (
              <>
                <div className="mb-6">
                  <h3 className="text-white text-lg mb-3">Multisig Transaction Result</h3>
                  <pre className="p-4 bg-white/10 rounded-lg text-white/90 overflow-auto text-sm">
                    {JSON.stringify(multisigCreateState.result, null, 2)}
                  </pre>
                </div>
                <button
                  onClick={resetMultisigCreate}
                  className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all"
                >
                  Create Another Multisig
                </button>
              </>
            )}
          </div>
        )}

        {/* Find Multisig Tab */}
        {activeTab === 'find' && (
          <div className="bg-white/10 backdrop-blur-md p-6 rounded-xl">
            <h2 className="text-2xl text-white mb-4">Find Multisig UTXOs</h2>
            {!findMultisigState.result ? (
              <>
                <div className="space-y-4 mb-6">
                  <textarea
                    placeholder="Public Keys (comma-separated)"
                    value={findMultisigState.pubkeys}
                    onChange={(e) => setFindMultisigState(prev => ({ ...prev, pubkeys: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none h-24"
                  />
                  <input
                    type="number"
                    placeholder="Required Signatures (m)"
                    value={findMultisigState.m}
                    onChange={(e) => setFindMultisigState(prev => ({ ...prev, m: parseInt(e.target.value) || 2 }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                </div>
                <button
                  onClick={handleFindMultisig}
                  disabled={submitting}
                  className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50 transition-all"
                >
                  {submitting ? 'Searching...' : 'Find UTXOs'}
                </button>
              </>
            ) : (
              <>
                <div className="mb-6">
                  <h3 className="text-white text-lg mb-3">Multisig UTXOs Found</h3>
                  {findMultisigState.result.multisig_utxos && findMultisigState.result.multisig_utxos.length > 0 ? (
                    <div className="space-y-3">
                      {findMultisigState.result.multisig_utxos.map((utxo, i) => (
                        <div key={i} className="p-4 bg-white/10 rounded-lg">
                          <p className="text-white/90 font-mono text-sm mb-2">
                            <span className="text-white/70">TXID:</span> {utxo.txid}
                          </p>
                          <p className="text-white/90">
                            <span className="text-white/70">Value:</span> <span className="text-green-300">{(utxo.value / 1e8).toFixed(8)} BTC</span>
                          </p>
                          {utxo.vout !== undefined && (
                            <p className="text-white/90">
                              <span className="text-white/70">Vout:</span> {utxo.vout}
                            </p>
                          )}
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="text-white/70 p-4 bg-white/10 rounded-lg">No multisig UTXOs found.</p>
                  )}
                </div>
                <button
                  onClick={resetFindMultisig}
                  className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all"
                >
                  Search Again
                </button>
              </>
            )}
          </div>
        )}

        {/* Create Spending Tab */}
        {activeTab === 'spending' && (
          <div className="bg-white/10 backdrop-blur-md p-6 rounded-xl">
            <h2 className="text-2xl text-white mb-4">Create Spending Transaction (Unsigned)</h2>
            {!spendingCreateState.result ? (
              <>
                <div className="space-y-4 mb-6">
                  <input
                    placeholder="TXID"
                    value={spendingCreateState.txid}
                    onChange={(e) => setSpendingCreateState(prev => ({ ...prev, txid: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <input
                    type="number"
                    placeholder="Vout"
                    value={spendingCreateState.vout}
                    onChange={(e) => setSpendingCreateState(prev => ({ ...prev, vout: parseInt(e.target.value) || 0 }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <textarea
                    placeholder="Public Keys (comma-separated)"
                    value={spendingCreateState.pubkeys}
                    onChange={(e) => setSpendingCreateState(prev => ({ ...prev, pubkeys: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none h-24"
                  />
                  <input
                    type="number"
                    placeholder="Required Signatures (m)"
                    value={spendingCreateState.m}
                    onChange={(e) => setSpendingCreateState(prev => ({ ...prev, m: parseInt(e.target.value) || 2 }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <input
                    placeholder="To Address"
                    value={spendingCreateState.toAddress}
                    onChange={(e) => setSpendingCreateState(prev => ({ ...prev, toAddress: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <input
                    placeholder="Value (BTC)"
                    value={spendingCreateState.value_btc}
                    onChange={(e) => setSpendingCreateState(prev => ({ ...prev, value_btc: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <input
                    placeholder="Fee (BTC)"
                    value={spendingCreateState.fee_btc}
                    onChange={(e) => setSpendingCreateState(prev => ({ ...prev, fee_btc: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                </div>
                <button
                  onClick={handleCreateSpending}
                  disabled={submitting}
                  className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:opacity-50 transition-all"
                >
                  {submitting ? 'Creating...' : 'Create Unsigned TX'}
                </button>
              </>
            ) : (
              <>
                <div className="mb-6">
                  <h3 className="text-white text-lg mb-3">Unsigned Transaction</h3>
                  <pre className="p-4 bg-white/10 rounded-lg text-white/90 overflow-auto text-sm mb-4">
                    {JSON.stringify(spendingCreateState.result.unsigned_transaction, null, 2)}
                  </pre>
                  <button
                    onClick={() => copyJson(spendingCreateState.result.unsigned_transaction)}
                    className="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all"
                  >
                    Copy JSON
                  </button>
                </div>
                <button
                  onClick={resetSpendingCreate}
                  className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all"
                >
                  Create Another Spending TX
                </button>
              </>
            )}
          </div>
        )}

        {/* Sign Tab */}
        {activeTab === 'sign' && (
          <div className="bg-white/10 backdrop-blur-md p-6 rounded-xl">
            <h2 className="text-2xl text-white mb-4">Sign Multisig Transaction</h2>
            {!signState.result ? (
              <>
                <div className="space-y-4 mb-6">
                  <input
                    placeholder="TXID"
                    value={signState.txid}
                    onChange={(e) => setSignState(prev => ({ ...prev, txid: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <input
                    type="number"
                    placeholder="Vout"
                    value={signState.vout}
                    onChange={(e) => setSignState(prev => ({ ...prev, vout: parseInt(e.target.value) || 0 }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <textarea
                    placeholder="Spending Transaction JSON"
                    value={signState.spendingTxJson}
                    onChange={(e) => setSignState(prev => ({ ...prev, spendingTxJson: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none h-32 font-mono text-sm"
                  />
                  <textarea
                    placeholder="Public Keys (comma-separated)"
                    value={signState.pubkeys}
                    onChange={(e) => setSignState(prev => ({ ...prev, pubkeys: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none h-24"
                  />
                  <input
                    type="number"
                    placeholder="Required Signatures (m)"
                    value={signState.m}
                    onChange={(e) => setSignState(prev => ({ ...prev, m: parseInt(e.target.value) || 2 }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                </div>
                <button
                  onClick={handleSignMultisig}
                  disabled={submitting}
                  className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:opacity-50 transition-all"
                >
                  {submitting ? 'Signing...' : 'Sign Transaction'}
                </button>
              </>
            ) : (
              <>
                <div className="mb-6">
                  <h3 className="text-white text-lg mb-3">Signature Result</h3>
                  <pre className="p-4 bg-white/10 rounded-lg text-white/90 overflow-auto text-sm">
                    {JSON.stringify(signState.result, null, 2)}
                  </pre>
                </div>
                <button
                  onClick={resetSign}
                  className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all"
                >
                  Sign Another Transaction
                </button>
              </>
            )}
          </div>
        )}

        {/* Spend Tab */}
        {activeTab === 'spend' && (
          <div className="bg-white/10 backdrop-blur-md p-6 rounded-xl">
            <h2 className="text-2xl text-white mb-4">Spend Multisig Transaction</h2>
            {!spendState.result ? (
              <>
                <div className="space-y-4 mb-6">
                  <textarea
                    placeholder="Spending Transaction JSON"
                    value={spendState.spendingTxJson}
                    onChange={(e) => setSpendState(prev => ({ ...prev, spendingTxJson: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none h-32 font-mono text-sm"
                  />
                  <textarea
                    placeholder="Public Keys (comma-separated)"
                    value={spendState.pubkeys}
                    onChange={(e) => setSpendState(prev => ({ ...prev, pubkeys: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none h-24"
                  />
                  <input
                    type="number"
                    placeholder="Required Signatures (m)"
                    value={spendState.m}
                    onChange={(e) => setSpendState(prev => ({ ...prev, m: parseInt(e.target.value) || 2 }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                  <textarea
                    placeholder="Signatures (comma-separated hex)"
                    value={spendState.sigs}
                    onChange={(e) => setSpendState(prev => ({ ...prev, sigs: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none h-24 font-mono text-sm"
                  />
                </div>
                <button
                  onClick={handleSpendMultisig}
                  disabled={submitting}
                  className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600 disabled:opacity-50 transition-all"
                >
                  {submitting ? 'Spending...' : 'Spend Multisig'}
                </button>
              </>
            ) : (
              <>
                <div className="mb-6">
                  <h3 className="text-white text-lg mb-3">Spend Result</h3>
                  <pre className="p-4 bg-white/10 rounded-lg text-white/90 overflow-auto text-sm">
                    {JSON.stringify(spendState.result, null, 2)}
                  </pre>
                </div>
                <button
                  onClick={resetSpend}
                  className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all"
                >
                  Spend Another Transaction
                </button>
              </>
            )}
          </div>
        )}

        {/* Check Balance Tab */}
        {activeTab === 'check' && (
          <div className="bg-white/10 backdrop-blur-md p-6 rounded-xl">
            <h2 className="text-2xl text-white mb-4">Check Address Balance</h2>
            {!checkBalanceState.result ? (
              <>
                <div className="space-y-4 mb-6">
                  <input
                    placeholder="Enter Bitcoin Address"
                    value={checkBalanceState.address}
                    onChange={(e) => setCheckBalanceState(prev => ({ ...prev, address: e.target.value }))}
                    className="w-full p-3 rounded-lg bg-white/10 text-white placeholder-white/50 border border-white/20 focus:border-white/40 focus:outline-none"
                  />
                </div>
                <button
                  onClick={handleCheckBalance}
                  disabled={submitting}
                  className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50 transition-all"
                >
                  {submitting ? 'Checking...' : 'Check Balance'}
                </button>
              </>
            ) : (
              <>
                <div className="mb-6">
                  <h3 className="text-white text-lg mb-3">Balance for {checkBalanceState.address}</h3>
                  <div className="p-6 bg-white/10 rounded-lg">
                    <p className="text-green-300 text-3xl font-bold mb-2">
                      {checkBalanceState.result.total_btc} BTC
                    </p>
                    <p className="text-white/70">
                      {checkBalanceState.result.total_satoshis} satoshis
                    </p>
                  </div>
                </div>
                <button
                  onClick={resetCheckBalance}
                  className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-all"
                >
                  Check Another Address
                </button>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

export default WalletPage
