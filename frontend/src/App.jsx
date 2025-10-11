import { BrowserRouter as Router, Routes, Route } from 'react-router-dom'
import Nav from './components/Nav'
import Home from './pages/Home'
import MiningPage from './pages/MiningPage'
import WalletPage from './pages/WalletPage'
import TreePage from './pages/TreePage'

function App() {
  return (
    <Router>
      <div className="min-h-screen bg-gradient-to-br from-purple-600 via-blue-600 to-indigo-800">
        <Nav />
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/mining" element={<MiningPage />} />
          <Route path="/wallet" element={<WalletPage />} />
          <Route path="/tree" element={<TreePage />} />
        </Routes>
      </div>
    </Router>
  )
}

export default App