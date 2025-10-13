import { useState, useEffect } from 'react';
import { ChevronRight, ChevronDown, X } from 'lucide-react';

function TreeNodeComponent({ node, level = 0, onToggle, expandedNodes, onNodeClick }) {
  const hasChildren = node.children && node.children.length > 0;
  const isExpanded = expandedNodes.has(node.qi);
  const levelColors = [
    'border-blue-400 bg-blue-900/20',
    'border-green-400 bg-green-900/20',
    'border-purple-400 bg-purple-900/20',
    'border-yellow-400 bg-yellow-900/20',
    'border-pink-400 bg-pink-900/20',
    'border-cyan-400 bg-cyan-900/20',
  ];
  const colorClass = levelColors[level % levelColors.length];

  const handleClick = (e) => {
    if (e.target.tagName === 'BUTTON') return; // Don't trigger on expand/collapse
    onNodeClick(node.hash);
  };

  return (
    <div className="ml-4">
      <div className="flex items-start gap-2 py-1" onClick={handleClick}>
        {hasChildren && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onToggle(node.qi);
            }}
            className="mt-1 p-0.5 hover:bg-white/10 rounded transition-colors"
          >
            {isExpanded ? (
              <ChevronDown className="w-4 h-4 text-white" />
            ) : (
              <ChevronRight className="w-4 h-4 text-white" />
            )}
          </button>
        )}
        {!hasChildren && <div className="w-5" />}

        <div className={`flex-1 border-l-4 ${colorClass} rounded px-3 py-2 cursor-pointer hover:bg-white/5 transition-colors`}>
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-mono text-sm text-white font-semibold">
              QI: {node.qi}
            </span>
            <span className="text-xs text-gray-400">|</span>
            <span className="font-mono text-xs text-gray-300">
              {node.hash.slice(0, 16)}...
            </span>
            {hasChildren && (
              <>
                <span className="text-xs text-gray-400">|</span>
                <span className="text-xs text-blue-300">
                  {node.children.length} child{node.children.length !== 1 ? 'ren' : ''}
                </span>
              </>
            )}
          </div>
        </div>
      </div>

      {hasChildren && isExpanded && (
        <div className="border-l-2 border-white/10 ml-2">
          {node.children.map((child) => (
            <TreeNodeComponent
              key={child.qi}
              node={child}
              level={level + 1}
              onToggle={onToggle}
              expandedNodes={expandedNodes}
              onNodeClick={onNodeClick}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function BlockDetails({ block, onClose }) {
  if (!block) return null;

  const formatValue = (sats) => `${(sats / 100000000).toFixed(8)} BTC (${sats} satoshis)`;

  const formatTimestamp = (ts) => new Date(ts).toLocaleString();

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="glass max-w-6xl max-h-[90vh] overflow-y-auto rounded-lg p-6 w-full">
        <div className="flex justify-between items-start mb-4">
          <h2 className="text-2xl font-bold text-blue-300">Block Details</h2>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-white transition-colors"
          >
            <X className="w-6 h-6" />
          </button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div className="space-y-2 text-white">
            <div className="text-white"><strong>Hash:</strong> <span className="font-mono text-sm">{block.hash}</span></div>
            <div className="text-white"><strong>Level:</strong> {block.level}</div>
            <div className="text-white"><strong>Position:</strong> {block.position}</div>
            <div className="text-white"><strong>Align:</strong> {block.align}</div>
            <div className="text-white"><strong>Version:</strong> {block.version}</div>
          </div>
          <div className="space-y-2 text-white">
            <div className="text-white"><strong>Parent Hash:</strong> <span className="font-mono text-sm">{block.parent_hash.slice(0, 16)}...</span></div>
            <div className="text-white"><strong>Merkle Root:</strong> <span className="font-mono text-sm">{block.merkle_root.slice(0, 16)}...</span></div>
            <div className="text-white"><strong>Bits:</strong> {block.bits}</div>
            <div className="text-white"><strong>Nonce:</strong> {block.nonce}</div>
            <div className="text-white"><strong>Timestamp:</strong> {formatTimestamp(block.timestamp)}</div>
          </div>
        </div>

        <details className="mb-6">
          <summary className="cursor-pointer text-blue-300 font-semibold mb-2">PQP Entry</summary>
          <div className="ml-4 p-3 bg-white/10 rounded border border-white/10">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2 text-white">
              <div className="text-white"><strong>Queue Index:</strong> {block.pqp_entry.queue_index}</div>
              <div className="text-white"><strong>Miner Address:</strong> {block.pqp_entry.miner_address}</div>
              <div className="text-white"><strong>Prev PQP Commitment:</strong> <span className="font-mono text-xs">{block.pqp_entry.prev_pqp_commitment.slice(0, 16)}...</span></div>
              <div className="text-white"><strong>Signature:</strong> <span className="font-mono text-xs">{block.pqp_entry.signature.slice(0, 32)}...</span></div>
            </div>
          </div>
        </details>

        <div>
          <h3 className="text-lg font-semibold text-white mb-4">PQP Commitment: <span className="font-mono text-sm">{block.pqp_commitment}</span></h3>

          <h3 className="text-lg font-semibold text-white mb-4">Transactions ({block.n_tx})</h3>
          <div className="space-y-4">
            {block.tx.map((tx, txIndex) => (
              <details key={txIndex} className="bg-white/10 rounded p-4 border border-white/10">
                <summary className="cursor-pointer font-semibold text-blue-300 mb-2">
                  Transaction {txIndex + 1}: {tx.txid.slice(0, 16)}...
                </summary>
                <div className="ml-4 space-y-4 mt-2">
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-2 text-white">
                    <div className="text-white"><strong>Version:</strong> {tx.version}</div>
                    <div className="text-white"><strong>Locktime:</strong> {tx.locktime}</div>
                    {tx.hash && <div className="text-white"><strong>Hash:</strong> {tx.hash.slice(0, 16)}...</div>}
                  </div>

                  <div>
                    <h4 className="font-semibold text-gray-300 mb-2">Inputs ({tx.vin.length})</h4>
                    <div className="space-y-1">
                      {tx.vin.map((vin, vinIndex) => (
                        <div key={vinIndex} className="text-xs p-2 bg-white/5 rounded hover:bg-white/10 text-white">
                          <div className="text-white"><strong>TxID:</strong> {vin.txid.slice(0, 16)}...</div>
                          <div className="text-white"><strong>Vout:</strong> {vin.vout}</div>
                          <div className="text-white"><strong>Script Sig:</strong> {vin.script_sig ? vin.script_sig.slice(0, 32) + '...' : 'N/A'}</div>
                          <div className="text-white"><strong>Sequence:</strong> {vin.sequence}</div>
                        </div>
                      ))}
                    </div>
                  </div>

                  <div>
                    <h4 className="font-semibold text-gray-300 mb-2">Outputs ({tx.vout.length})</h4>
                    <div className="space-y-1">
                      {tx.vout.map((vout, voutIndex) => (
                        <div key={voutIndex} className="text-xs p-2 bg-white/5 rounded hover:bg-white/10 text-white">
                          <div className="text-white"><strong>Value:</strong> {formatValue(vout.value)}</div>
                          <div className="text-white"><strong>Script Pubkey:</strong> {vout.script_pubkey.slice(0, 32)}...</div>
                        </div>
                      ))}
                    </div>
                  </div>

                  {tx.witnesses && (
                    <div>
                      <h4 className="font-semibold text-gray-300 mb-2">Witnesses</h4>
                      <pre className="text-xs bg-white/5 p-2 rounded overflow-auto border border-white/10 text-white">{JSON.stringify(tx.witnesses, null, 2)}</pre>
                    </div>
                  )}
                </div>
              </details>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function LevelView({ treeData, onNodeClick }) {
  const [selectedLevel, setSelectedLevel] = useState(0);
  const [levelMap, setLevelMap] = useState(new Map());
  const [maxLevel, setMaxLevel] = useState(0);

  useEffect(() => {
    if (!treeData) return;

    const map = new Map();

    function traverse(node, level) {
      if (!map.has(level)) {
        map.set(level, []);
      }
      map.get(level).push(node);

      if (node.children && node.children.length > 0) {
        node.children.forEach(child => traverse(child, level + 1));
      }
    }

    traverse(treeData, 0);

    // Sort nodes in each level by queue_index (qi)
    const sortedLevelMap = new Map();
    for (const [lvl, nodes] of map.entries()) {
      const sorted = [...nodes].sort((a, b) => a.qi - b.qi);
      sortedLevelMap.set(lvl, sorted);
    }

    setLevelMap(sortedLevelMap);
    setMaxLevel(Math.max(...Array.from(sortedLevelMap.keys())));
  }, [treeData]);

  const currentLevelNodes = levelMap.get(selectedLevel) || [];

  return (
    <div className="space-y-6">
      <div className="glass p-4 rounded-lg">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-white">Level Explorer</h3>
          <div className="text-sm text-gray-300">
            Total Levels: {maxLevel + 1} | Nodes in Level: {currentLevelNodes.length}
          </div>
        </div>

        <div className="flex items-center gap-4">
          <button
            onClick={() => setSelectedLevel(Math.max(0, selectedLevel - 1))}
            disabled={selectedLevel === 0}
            className="px-4 py-2 bg-blue-600 text-white rounded disabled:opacity-50 disabled:cursor-not-allowed hover:bg-blue-700 transition-colors"
          >
            Previous Level
          </button>

          <div className="flex-1">
            <input
              type="range"
              min="0"
              max={maxLevel}
              value={selectedLevel}
              onChange={(e) => setSelectedLevel(parseInt(e.target.value))}
              className="w-full"
            />
            <div className="text-center text-white font-semibold mt-2">
              Level {selectedLevel}
            </div>
          </div>

          <button
            onClick={() => setSelectedLevel(Math.min(maxLevel, selectedLevel + 1))}
            disabled={selectedLevel === maxLevel}
            className="px-4 py-2 bg-blue-600 text-white rounded disabled:opacity-50 disabled:cursor-not-allowed hover:bg-blue-700 transition-colors"
          >
            Next Level
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
        {currentLevelNodes.map((node) => (
          <div
            key={node.qi}
            onClick={() => onNodeClick(node.hash)}
            className="glass p-4 rounded-lg border-l-4 border-blue-400 cursor-pointer hover:bg-white/5 transition-colors"
          >
            <div className="font-mono text-lg text-blue-300 font-bold mb-2">
              QI: {node.qi}
            </div>
            <div className="font-mono text-xs text-gray-300 break-all mb-2">
              {node.hash.slice(0, 32)}...
            </div>
            <div className="text-xs text-gray-400">
              Children: {node.children?.length || 0}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function TreeView({ treeData, onNodeClick }) {
  const [expandedNodes, setExpandedNodes] = useState(new Set([0]));
  const [expandAll, setExpandAll] = useState(false);

  const handleToggle = (qi) => {
    const newExpanded = new Set(expandedNodes);
    if (newExpanded.has(qi)) {
      newExpanded.delete(qi);
    } else {
      newExpanded.add(qi);
    }
    setExpandedNodes(newExpanded);
  };

  const handleExpandAll = () => {
    if (expandAll || !treeData) {
      setExpandedNodes(new Set());
      setExpandAll(false);
    } else {
      const allNodes = new Set();
      function collectNodes(node) {
        allNodes.add(node.qi);
        if (node.children) {
          node.children.forEach(child => collectNodes(child));
        }
      }
      collectNodes(treeData);
      setExpandedNodes(allNodes);
      setExpandAll(true);
    }
  };

  if (!treeData) {
    return <p className="text-gray-400">Loading tree data...</p>;
  }

  return (
    <div className="space-y-4">
      <div className="flex justify-between items-center">
        <h2 className="text-2xl font-semibold text-white">Tree Structure (From Genesis)</h2>
        <button
          onClick={handleExpandAll}
          className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 transition-colors"
        >
          {expandAll ? 'Collapse All' : 'Expand All'}
        </button>
      </div>

      <div className="glass p-6 rounded-lg max-h-[600px] overflow-y-auto">
        <TreeNodeComponent
          node={treeData}
          level={0}
          onToggle={handleToggle}
          expandedNodes={expandedNodes}
          onNodeClick={onNodeClick}
        />
      </div>
    </div>
  );
}

function QueueIndexView({ blocks, onNodeClick }) {
  return (
    <div className="space-y-4">
      <h2 className="text-2xl font-semibold text-white">
        Blocks in Queue Index Order (First 50)
      </h2>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {blocks.map((block, index) => (
          <div
            key={index}
            onClick={() => onNodeClick(block.hash)}
            className="glass p-4 rounded-lg cursor-pointer hover:bg-white/5 transition-colors"
          >
            <h3 className="font-mono text-sm text-blue-300 font-bold mb-2">
              Queue Index: {block.pqp_entry?.queue_index}
            </h3>
            <p className="font-mono text-xs text-gray-300 break-all mb-1">
              {block.hash.slice(0, 32)}...
            </p>
            <p className="text-xs text-gray-400">Level: {block.level}</p>
            <p className="text-xs text-gray-400">Position: {block.position}</p>
            <p className="text-xs text-gray-400">Transactions: {block.n_tx}</p>
          </div>
        ))}
      </div>
      {blocks.length === 0 && <p className="text-gray-400">No blocks loaded.</p>}
    </div>
  );
}

function PQPEntryCard({ entry, onClick }) {
  return (
    <div
      onClick={() => onClick(entry.block_hash)}
      className="glass p-4 rounded-lg cursor-pointer hover:bg-white/5 transition-colors min-w-[300px] flex-shrink-0"
    >
      <div className="font-mono text-sm text-blue-300 font-bold mb-2">
        QI: {entry.queue_index}
      </div>
      <div className="font-mono text-xs text-gray-300 break-all mb-1">
        Block Hash: {entry.block_hash.slice(0, 16)}...
      </div>
      <div className="text-xs text-gray-400 mb-1">Align: {entry.align}</div>
      <div className="text-xs text-gray-400 mb-1">Miner: {entry.miner_address}</div>
      <div className="text-xs text-gray-400 mb-1">Parent: {entry.parent_hash.slice(0, 16)}...</div>
      <div className="text-xs text-gray-400">Prev Commitment: {entry.prev_pqp_commitment.slice(0, 16)}...</div>
    </div>
  );
}

function PQPView({ pqpEntries, onNodeClick }) {
  if (!pqpEntries || pqpEntries.length === 0) {
    return <p className="text-gray-400">Loading PQP data...</p>;
  }

  const currentParent = pqpEntries[0];
  const nextParent = pqpEntries[1] || null;

  // Calculate current siblings: from last entry backwards with same parent_hash
  const lastEntry = pqpEntries[pqpEntries.length - 1];
  const siblings = [];
  let i = pqpEntries.length - 1;
  while (i >= 0) {
    if (pqpEntries[i].parent_hash === lastEntry.parent_hash) {
      siblings.unshift(pqpEntries[i]);
    } else {
      break;
    }
    i--;
  }

  return (
    <div className="flex flex-col space-x-4">
      {/* Scrollable entries list */}
      <div className="flex-1 overflow-x-auto space-x-4 pb-4" style={{scrollbarWidth:'none'}}>
        <h2 className="text-2xl font-semibold text-white mb-4">PQP Entries</h2>
        <div className="flex space-x-4">
          {pqpEntries.map((entry, index) => (
            <PQPEntryCard key={index} entry={entry} onClick={onNodeClick} />
          ))}
        </div>
      </div>

      {/* bottom panel with summaries */}
<div className="w-full flex flex-wrap justify-around gap-6 pr-6 mt-2">
  <div className="flex-1 min-w-[300px] glass p-4 rounded-lg">
    <h3 className="text-lg font-semibold text-white mb-4">Current Parent</h3>
    {currentParent && (
      <div className="text-white space-y-2">
        <div><strong>QI:</strong> {currentParent.queue_index}</div>
        <div><strong>Block Hash:</strong> {currentParent.block_hash.slice(0, 16)}...</div>
        <div><strong>Miner:</strong> {currentParent.miner_address}</div>
        <div><strong>Parent Hash:</strong> {currentParent.parent_hash.slice(0, 16)}...</div>
      </div>
    )}
  </div>

  <div className="flex-1 min-w-[300px] glass p-4 rounded-lg">
    <h3 className="text-lg font-semibold text-white mb-4">Next Parent</h3>
    {nextParent ? (
      <div className="text-white space-y-2">
        <div><strong>QI:</strong> {nextParent.queue_index}</div>
        <div><strong>Block Hash:</strong> {nextParent.block_hash.slice(0, 16)}...</div>
        <div><strong>Miner:</strong> {nextParent.miner_address}</div>
        <div><strong>Parent Hash:</strong> {nextParent.parent_hash.slice(0, 16)}...</div>
      </div>
    ) : (
      <p className="text-gray-400">No next parent</p>
    )}
  </div>

  <div className="flex-1 min-w-[300px] glass p-4 rounded-lg">
    <h3 className="text-lg font-semibold text-white mb-4">
      Current Siblings ({siblings.length})
    </h3>
    <div className="space-y-2 max-h-64 overflow-y-auto" style={{scrollbarWidth:'none'}}>
      {siblings.map((sib, index) => (
        <div key={index} className="text-white text-xs p-2 bg-white/5 rounded">
          <div><strong>QI:</strong> {sib.queue_index}</div>
          <div><strong>Hash:</strong> {sib.block_hash.slice(0, 16)}...</div>
        </div>
      ))}
    </div>
  </div>
</div>

    </div>
  );
}

function TreePage() {
  const [view, setView] = useState('queue');
  const [blocks, setBlocks] = useState([]);
  const [treeData, setTreeData] = useState(null);
  const [pqpEntries, setPQPEntries] = useState([]);
  const [loading, setLoading] = useState(false);
  const [selectedBlock, setSelectedBlock] = useState(null);
  const apiBase = import.meta.env.VITE_API_BASE;

  const fetchBlock = async (hash) => {
    try {
      const res = await fetch(`${apiBase}/get_block/${hash}`);
      const data = await res.json();
      if (data.status === 'success' && data.block) {
        setSelectedBlock(data.block);
      } else {
        console.error('Failed to fetch block');
      }
    } catch (err) {
      console.error('Error fetching block:', err);
    }
  };

  useEffect(() => {
    setLoading(true);

    if (view === 'queue') {
      fetch(`${apiBase}/get_blocks`)
        .then((res) => res.json())
        .then((data) => {
          if (data.status === 'success' && data.blocks) {
            const sortedBlocks = data.blocks
              .filter((block) => block.pqp_entry && block.position !== '')
              .sort((a, b) =>
                (a.pqp_entry?.queue_index || 0) - (b.pqp_entry?.queue_index || 0)
              )
              .slice(0, 50);
            setBlocks(sortedBlocks);
          }
          setTreeData(null);
          setPQPEntries([]);
        })
        .catch((err) => console.error('Error fetching blocks:', err))
        .finally(() => setLoading(false));
    } else if (view === 'pqp') {
      fetch(`${apiBase}/get_pqp`)
        .then((res) => res.json())
        .then((data) => {
          if (data.status === 'success' && data.blocks) {
            setPQPEntries(data.blocks);
          }
          setBlocks([]);
          setTreeData(null);
        })
        .catch((err) => console.error('Error fetching PQP:', err))
        .finally(() => setLoading(false));
    } else {
      fetch(`${apiBase}/children_map`)
        .then((res) => res.json())
        .then((data) => {
          if (data.status === 'success' && data.children_map) {
            const childrenMap = new Map();

            data.children_map.forEach((entry) => {
              const children = entry.children
                .map(([hash, qi]) => ({
                  hash,
                  qi,
                  children: []
                }))
                .sort((a, b) => a.qi - b.qi);
              childrenMap.set(entry.parent_hash, children);
            });

            const buildTree = (hash, qi) => {
              const children = childrenMap.get(hash) || [];
              const node = {
                hash,
                qi,
                children: children.map(child => buildTree(child.hash, child.qi))
              };
              return node;
            };

            const genesisHash = '0000181c51c930a46ede1edbd3082c0e0d3673334fac3ddc60262c66a2c46b22';
            const root = buildTree(genesisHash, 0);
            setTreeData(root);
          }
          setBlocks([]);
          setPQPEntries([]);
        })
        .catch((err) => console.error('Error fetching children map:', err))
        .finally(() => setLoading(false));
    }
  }, [view]);

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-blue-900 to-gray-900 pt-20 pb-10 px-4">
      <div className="max-w-7xl mx-auto">
        <h1 className="text-4xl font-bold text-white mb-8 text-center animate-fade-in">
          TreeChain Block Explorer
        </h1>

        <div className="flex justify-center mb-8 space-x-4">
          <button
            onClick={() => setView('queue')}
            className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${
              view === 'queue'
                ? 'bg-blue-600 text-white shadow-lg'
                : 'bg-transparent border-2 border-blue-500 text-blue-300 hover:bg-blue-500/20'
            }`}
          >
            Queue Index Order
          </button>
          <button
            onClick={() => setView('tree')}
            className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${
              view === 'tree'
                ? 'bg-green-600 text-white shadow-lg'
                : 'bg-transparent border-2 border-green-500 text-green-300 hover:bg-green-500/20'
            }`}
          >
            Tree View
          </button>
          <button
            onClick={() => setView('levels')}
            className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${
              view === 'levels'
                ? 'bg-purple-600 text-white shadow-lg'
                : 'bg-transparent border-2 border-purple-500 text-purple-300 hover:bg-purple-500/20'
            }`}
          >
            Level View
          </button>
          <button
            onClick={() => setView('pqp')}
            className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${
              view === 'pqp'
                ? 'bg-indigo-600 text-white shadow-lg'
                : 'bg-transparent border-2 border-indigo-500 text-indigo-300 hover:bg-indigo-500/20'
            }`}
          >
            PQP
          </button>
        </div>

        {loading ? (
          <div className="glass p-8 rounded-xl text-center text-white/80">
            <p>Loading...</p>
          </div>
        ) : (
          <div>
            {view === 'queue' && <QueueIndexView blocks={blocks} onNodeClick={fetchBlock} />}
            {view === 'tree' && <TreeView treeData={treeData} onNodeClick={fetchBlock} />}
            {view === 'levels' && <LevelView treeData={treeData} onNodeClick={fetchBlock} />}
            {view === 'pqp' && <PQPView pqpEntries={pqpEntries} onNodeClick={fetchBlock} />}
          </div>
        )}
      </div>

      {selectedBlock && (
        <BlockDetails
          block={selectedBlock}
          onClose={() => setSelectedBlock(null)}
        />
      )}
    </div>
  );
}

export default TreePage;