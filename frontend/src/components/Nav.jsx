import { Link, useLocation } from 'react-router-dom'

function Nav() {
  const location = useLocation()

  const isActive = (path) => location.pathname === path ? 'bg-white/20' : ''

  return (
    <nav className="glass px-6 py-4 flex justify-between items-center shadow-lg">
      <Link to="/" className="text-2xl font-bold text-white">🌳 TreeChain</Link>
      <div className="space-x-4">
        <Link to="/" className={`px-3 py-2 rounded-lg text-white hover:bg-white/10 transition-all ${isActive('/')}`}>Home</Link>
        <Link to="/mining" className={`px-3 py-2 rounded-lg text-white hover:bg-white/10 transition-all ${isActive('/mining')}`}>Mining</Link>
        <Link to="/wallet" className={`px-3 py-2 rounded-lg text-white hover:bg-white/10 transition-all ${isActive('/wallet')}`}>Wallet</Link>
        <Link to="/tree" className={`px-3 py-2 rounded-lg text-white hover:bg-white/10 transition-all ${isActive('/tree')}`}>Tree</Link>
      </div>
    </nav>
  )
}

export default Nav