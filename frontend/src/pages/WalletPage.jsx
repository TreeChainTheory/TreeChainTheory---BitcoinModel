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
  const [formData, setFormData] = useState({
    // Normal Txn (now in BTC)
    toAddress: '',
    value_btc: '',
    fee_btc: '',
    // Multisig Create (now in BTC)
    pubkeys: '',
    m: 2,
    multisigValue_btc: '',
    multisigFee_btc: '',
    // Find Multisig
    findPubkeys: '',
    findM: 2,
    // Create Spending (now in BTC)
    spendTxid: '',
    spendVout: 0,
    spendPubkeys: '',
    spendM: 2,
    spendToAddress: '',
    spendValue_btc: '',
    spendFee_btc: '',
    unsignedTx: null,
    // Sign
    signTxid: '',
    signVout: 0,
    spendingTxJson: '',
    signPubkeys: '',
    signM: 2,
    signedSig: '',
    // Spend
    spendSpendingTxJson: '',
    spendPubkeys: '',
    spendM: 2,
    sigs: '',
    // Check Balance
    checkAddress: ''
  })
  const [results, setResults] = useState({})
  const apiBase = import.meta.env.VITE_API_BASE

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
    } catch (err) {
      console.error(err)
    }
    setLoading(false)
  }

  const handleInputChange = (e, section) => {
    const { name, value } = e.target
    setFormData(prev => ({ ...prev, [section]: { ...prev[section], [name]: value } }))
  }

  // For arrays/textareas
  const handleTextareaChange = (e, key) => {
    setFormData(prev => ({ ...prev, [key]: e.target.value }))
  }

  const copyJson = (json) => {
    navigator.clipboard.writeText(JSON.stringify(json, null, 2))
    alert('Copied to clipboard!')
  }

  const submitForm = async (endpoint, body, resultKey) => {
    try {
        console.log('Submitting to', endpoint, 'with body', body ) // Debugging line
      const res = await fetch(`${apiBase}/${endpoint}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body)
      })
      const data = await res.json()
      console.log('Response from', endpoint, 'is', data) // Debugging line
      setResults(prev => ({ ...prev, [resultKey]: data }))
    } catch (err) {
      console.error(err)
      alert(`Error: ${err.message}`)
    }
  }

  // Helper to convert BTC to sat
  const btcToSat = (btc) => Math.floor(parseFloat(btc) * 100000000)

  const handleNormalTxn = () => {
    const body = {
      to_address: formData.toAddress,
      value: btcToSat(formData.value_btc),
      fee: btcToSat(formData.fee_btc)
    }
    submitForm('create_txn', body, 'normalTxn')
  }

  const handleMultisigCreate = () => {
    const body = {
      pubkeys: formData.pubkeys.split(',').map(p => p.trim()).filter(p => p.length > 0),
      m: formData.m,
      value: btcToSat(formData.multisigValue_btc),
      fee: btcToSat(formData.multisigFee_btc)
    }
    submitForm('create_multisig_txn', body, 'multisigCreate')
  }

  const handleFindMultisig = () => {
    const body = {
      pubkeys: formData.findPubkeys.split(',').map(p => p.trim()).filter(p => p.length > 0),
      m: formData.findM
    }
    fetch(`${apiBase}/find_multisig_utxo`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    })
      .then(res => res.json())
      .then(data => setResults(prev => ({ ...prev, findMultisig: data.multisig_utxos })))
  }

  const handleCreateSpending = () => {
    console.log('Creating spending multisig with data:', formData) // Debugging line
    const body = {
      txid: formData.spendTxid,
      vout: formData.spendVout,
      pubkeys: formData.spendPubkeys.split(',').map(p => p.trim()).filter(p => p.length > 0),
      m: formData.spendM,
      to_address: formData.spendToAddress,
      value: btcToSat(formData.spendValue_btc),
      fee: btcToSat(formData.spendFee_btc)
    }
    submitForm('create_spending_multisig_tx', body, 'spendingCreate')
  }

  const handleSignMultisig = () => {
    const body = {
      txid: formData.signTxid,
      vout: formData.signVout,
      spending_tx: JSON.parse(formData.spendingTxJson),
      pubkeys: formData.signPubkeys.split(',').map(p => p.trim()),
      m: formData.signM
    }
    submitForm('sign_multisig', body, 'signResult')
  }

  const handleSpendMultisig = () => {
    const body = {
      spending_tx: JSON.parse(formData.spendSpendingTxJson),
      pubkeys: formData.spendPubkeys.split(',').map(p => p.trim()),
      m: formData.spendM,
      sigs: formData.sigs.split(',').map(s => s.trim())
    }
    submitForm('spend_multisig_txn', body, 'spendResult')
  }

  const handleCheckBalance = () => {
    const address = formData.checkAddress
    if (!address) {
      alert('Please enter an address')
      return
    }
    fetch(`${apiBase}/balance/${address}`)
      .then(res => res.json())
      .then(data => {
        if (data.status === 'success') {
          setResults(prev => ({ ...prev, checkBalance: data.balance }))
        } else {
          alert('Error fetching balance')
        }
      })
      .catch(err => alert(`Error: ${err.message}`))
  }

  if (loading) return <div className="text-white text-center pt-20">Loading...</div>

  return (
    <div className="pt-20 pb-10 px-4 max-w-6xl mx-auto">
      <h1 className="text-4xl font-bold text-white mb-8 text-center animate-fade-in">Wallet Dashboard</h1>
      
      {/* Balance */}
      <div className={`glass p-6 rounded-xl mb-8 ${activeTab === 'balance' ? '' : 'hidden'}`}>
        <h2 className="text-2xl text-white mb-4">Balance</h2>
        <p className="text-white/90 mb-2">Address: <span className="font-mono bg-white/10 px-2 py-1 rounded break-all">{wallet.address}</span></p>
        <p className="text-white/90 mb-2">Public Key: <span className="font-mono bg-white/10 px-2 py-1 rounded break-all">{wallet.public_key}</span></p>
        <p className="text-3xl text-green-300 mt-2">Balance: {wallet.balance_btc} BTC</p>
        <button onClick={fetchWallet} className="mt-4 px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600">Refresh</button>
      </div>

      {/* Tabs */}
      <div className="glass p-4 rounded-xl mb-8 flex space-x-4 overflow-x-auto">
        {['balance', 'normal', 'multisig', 'spending', 'sign', 'spend', 'check_balance'].map(tab => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`px-4 py-2 rounded-lg transition-all ${activeTab === tab ? 'bg-white/20' : 'text-white/70 hover:bg-white/10'}`}
          >
            {tab === 'check_balance' ? 'Check Balance' : tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>

      {/* Normal Txn */}
      <div className={`glass p-6 rounded-xl mb-8 ${activeTab === 'normal' ? '' : 'hidden'}`}>
        <h2 className="text-2xl text-white mb-4">Create Normal Transaction</h2>
        <div className="grid md:grid-cols-3 gap-4 mb-4">
          <input
            name="toAddress"
            placeholder="To Address"
            value={formData.toAddress}
            onChange={(e) => setFormData({...formData, toAddress: e.target.value})}
            className="p-2 rounded bg-white/10 text-white"
          />
          <input
            name="value_btc"
            placeholder="Value (BTC)"
            value={formData.value_btc}
            onChange={(e) => setFormData({...formData, value_btc: e.target.value})}
            className="p-2 rounded bg-white/10 text-white"
          />
          <input
            name="fee_btc"
            placeholder="Fee (BTC)"
            value={formData.fee_btc}
            onChange={(e) => setFormData({...formData, fee_btc: e.target.value})}
            className="p-2 rounded bg-white/10 text-white"
          />
        </div>
        <button onClick={handleNormalTxn} className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600">Send</button>
        {results.normalTxn && (
          <pre className="mt-4 p-4 bg-white/10 rounded text-white/90 overflow-auto">{JSON.stringify(results.normalTxn, null, 2)}</pre>
        )}
      </div>

      {/* Multisig Create */}
      <div className={`glass p-6 rounded-xl mb-8 ${activeTab === 'multisig' ? '' : 'hidden'}`}>
        <h2 className="text-2xl text-white mb-4">Create Multisig Transaction</h2>
        <textarea
          placeholder="Pubkeys (comma-separated)"
          value={formData.pubkeys}
          onChange={(e) => setFormData({...formData, pubkeys: e.target.value})}
          className="w-full p-2 rounded bg-white/10 text-white mb-4 h-20"
        />
        <input name="m" type="number" placeholder="m" value={formData.m} onChange={(e) => setFormData({...formData, m: parseInt(e.target.value)})} className="p-2 rounded bg-white/10 text-white mb-2" />
        <div className="grid grid-cols-2 gap-4 mb-4">
          <input placeholder="Value (BTC)" value={formData.multisigValue_btc} onChange={(e) => setFormData({...formData, multisigValue_btc: e.target.value})} className="p-2 rounded bg-white/10 text-white" />
          <input placeholder="Fee (BTC)" value={formData.multisigFee_btc} onChange={(e) => setFormData({...formData, multisigFee_btc: e.target.value})} className="p-2 rounded bg-white/10 text-white" />
        </div>
        <button onClick={handleMultisigCreate} className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600">Create</button>
        {results.multisigCreate && <pre className="mt-4 p-4 bg-white/10 rounded text-white/90 overflow-auto">{JSON.stringify(results.multisigCreate, null, 2)}</pre>}
      </div>

      {/* Find Multisig UTXOs */}
      <div className={`glass p-6 rounded-xl mb-8 ${activeTab === 'multisig' ? '' : 'hidden'}`}>
        <h3 className="text-xl text-white mb-2">Find Multisig UTXOs</h3>
        <textarea placeholder="Pubkeys (comma-separated)" value={formData.findPubkeys} onChange={(e) => setFormData({...formData, findPubkeys: e.target.value})} className="w-full p-2 rounded bg-white/10 text-white mb-2 h-20" />
        <input name="findM" type="number" placeholder="m" value={formData.findM} onChange={(e) => setFormData({...formData, findM: parseInt(e.target.value)})} className="p-2 rounded bg-white/10 text-white mb-4" />
        <button onClick={handleFindMultisig} className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600">Find</button>
        {
            results.findMultisig && results.findMultisig.length === 0 && <p className="mt-4 text-white/80">No multisig UTXOs found.</p>
        }
        {results.findMultisig && results.findMultisig.length >0 && (
          <ul className="mt-4 space-y-2">
            {results.findMultisig.map((utxo, i) => (
              <li key={i} className="text-white/90 p-2 bg-white/10 rounded">TXID: {utxo.txid}, Value: {(utxo.value / 1e8).toFixed(8)} BTC</li>
            ))}
          </ul>
        )}
      </div>

      {/* Create Spending Multisig */}
      <div className={`glass p-6 rounded-xl mb-8 ${activeTab === 'spending' ? '' : 'hidden'}`}>
        <h2 className="text-2xl text-white mb-4">Create Spending Multisig TX (Unsigned)</h2>
        <div className="grid md:grid-cols-2 gap-4 mb-4">
          <input placeholder="TXID" value={formData.spendTxid} onChange={(e) => setFormData({...formData, spendTxid: e.target.value})} className="p-2 rounded bg-white/10 text-white" />
          <input type="number" placeholder="Vout" value={formData.spendVout} onChange={(e) => setFormData({...formData, spendVout: parseInt(e.target.value)})} className="p-2 rounded bg-white/10 text-white" />
        </div>
        <textarea placeholder="Pubkeys (comma-separated)" value={formData.spendPubkeys} onChange={(e) => setFormData({...formData, spendPubkeys: e.target.value})} className="w-full p-2 rounded bg-white/10 text-white mb-2 h-20" />
        <input name="spendM" type="number" placeholder="m" value={formData.spendM} onChange={(e) => setFormData({...formData, spendM: parseInt(e.target.value)})} className="p-2 rounded bg-white/10 text-white mb-2" />
        <div className="grid md:grid-cols-3 gap-4 mb-4">
          <input placeholder="To Address" value={formData.spendToAddress} onChange={(e) => setFormData({...formData, spendToAddress: e.target.value})} className="p-2 rounded bg-white/10 text-white" />
          <input placeholder="Value (BTC)" value={formData.spendValue_btc} onChange={(e) => setFormData({...formData, spendValue_btc: e.target.value})} className="p-2 rounded bg-white/10 text-white" />
          <input placeholder="Fee (BTC)" value={formData.spendFee_btc} onChange={(e) => setFormData({...formData, spendFee_btc: e.target.value})} className="p-2 rounded bg-white/10 text-white" />
        </div>
        <button onClick={handleCreateSpending} className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600">Create Unsigned</button>
        {
            results.spendingCreate && results.spendingCreate.status === 'error' && (
                <p className="mt-4 text-red-400">Error: {results.spendingCreate.message}</p>
            )
        }
        {results.spendingCreate && results.spendingCreate.unsigned_transaction && (
          <div className="mt-4 p-4 bg-white/10 rounded">
            <h3 className="text-white mb-2">Unsigned Transaction</h3>
            <pre className="text-white/90 overflow-auto">{JSON.stringify(results.spendingCreate.unsigned_transaction, null, 2)}</pre>
            <button onClick={() => copyJson(results.spendingCreate.unsigned_transaction)} className="mt-2 px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600">Copy JSON</button>
          </div>
        )}
      </div>

      {/* Sign Multisig */}
      <div className={`glass p-6 rounded-xl mb-8 ${activeTab === 'sign' ? '' : 'hidden'}`}>
        <h2 className="text-2xl text-white mb-4">Sign Multisig</h2>
        <div className="grid md:grid-cols-2 gap-4 mb-4">
          <input placeholder="TXID" value={formData.signTxid} onChange={(e) => setFormData({...formData, signTxid: e.target.value})} className="p-2 rounded bg-white/10 text-white" />
          <input type="number" placeholder="Vout" value={formData.signVout} onChange={(e) => setFormData({...formData, signVout: parseInt(e.target.value)})} className="p-2 rounded bg-white/10 text-white" />
        </div>
        <textarea placeholder="Spending TX JSON" value={formData.spendingTxJson} onChange={(e) => handleTextareaChange(e, 'spendingTxJson')} className="w-full p-2 rounded bg-white/10 text-white mb-2 h-32" />
        <textarea placeholder="Pubkeys (comma-separated)" value={formData.signPubkeys} onChange={(e) => setFormData({...formData, signPubkeys: e.target.value})} className="w-full p-2 rounded bg-white/10 text-white mb-2 h-20" />
        <input name="signM" type="number" placeholder="m" value={formData.signM} onChange={(e) => setFormData({...formData, signM: parseInt(e.target.value)})} className="p-2 rounded bg-white/10 text-white mb-4" />
        <button onClick={handleSignMultisig} className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600">Sign</button>
        {results.signResult && <pre className="mt-4 p-4 bg-white/10 rounded text-white/90 overflow-auto">{JSON.stringify(results.signResult, null, 2)}</pre>}
      </div>

      {/* Spend Multisig */}
      <div className={`glass p-6 rounded-xl mb-8 ${activeTab === 'spend' ? '' : 'hidden'}`}>
        <h2 className="text-2xl text-white mb-4">Spend Multisig TX</h2>
        <textarea placeholder="Spending TX JSON" value={formData.spendSpendingTxJson} onChange={(e) => handleTextareaChange(e, 'spendSpendingTxJson')} className="w-full p-2 rounded bg-white/10 text-white mb-2 h-32" />
        <textarea placeholder="Pubkeys (comma-separated)" value={formData.spendPubkeys} onChange={(e) => setFormData({...formData, spendPubkeys: e.target.value})} className="w-full p-2 rounded bg-white/10 text-white mb-2 h-20" />
        <input name="spendM" type="number" placeholder="m" value={formData.spendM} onChange={(e) => setFormData({...formData, spendM: parseInt(e.target.value)})} className="p-2 rounded bg-white/10 text-white mb-2" />
        <textarea placeholder="Sigs (comma-separated hex)" value={formData.sigs} onChange={(e) => setFormData({...formData, sigs: e.target.value})} className="w-full p-2 rounded bg-white/10 text-white mb-4 h-20" />
        <button onClick={handleSpendMultisig} className="px-6 py-3 bg-green-500 text-white rounded-lg hover:bg-green-600">Spend</button>
        {results.spendResult && <pre className="mt-4 p-4 bg-white/10 rounded text-white/90 overflow-auto">{JSON.stringify(results.spendResult, null, 2)}</pre>}
      </div>

      {/* Check Balance */}
      <div className={`glass p-6 rounded-xl mb-8 ${activeTab === 'check_balance' ? '' : 'hidden'}`}>
        <h2 className="text-2xl text-white mb-4">Check Address Balance</h2>
        <input
          placeholder="Enter Address"
          value={formData.checkAddress}
          onChange={(e) => setFormData({...formData, checkAddress: e.target.value})}
          className="w-full p-2 rounded bg-white/10 text-white mb-4"
        />
        <button onClick={handleCheckBalance} className="px-6 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600">Check Balance</button>
        {results.checkBalance && (
          <div className="mt-4 p-4 bg-white/10 rounded">
            <h3 className="text-white mb-2">Balance for {formData.checkAddress}</h3>
            <p className="text-green-300 text-xl">{results.checkBalance.total_btc} BTC</p>
            <p className="text-white/80">{results.checkBalance.total_satoshis} satoshis</p>
          </div>
        )}
      </div>
    </div>
  )
}

export default WalletPage