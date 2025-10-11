import { useState, useEffect } from 'react';
import { ChevronRight, ChevronDown } from 'lucide-react';

function TreeNodeComponent({ node, level = 0, onToggle, expandedNodes }) {
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

  return (
    <div className="ml-4">
      <div className="flex items-start gap-2 py-1">
        {hasChildren && (
          <button
            onClick={() => onToggle(node.qi)}
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

        <div className={`flex-1 border-l-4 ${colorClass} rounded px-3 py-2`}>
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
            />
          ))}
        </div>
      )}
    </div>
  );
}

function LevelView({ treeData }) {
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
    setLevelMap(map);
    setMaxLevel(Math.max(...Array.from(map.keys())));
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
          <div key={node.qi} className="glass p-4 rounded-lg border-l-4 border-blue-400">
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

function TreeView({ treeData }) {
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
        />
      </div>
    </div>
  );
}

function QueueIndexView({ blocks }) {
  return (
    <div className="space-y-4">
      <h2 className="text-2xl font-semibold text-white">
        Blocks in Queue Index Order (First 50)
      </h2>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {blocks.map((block, index) => (
          <div key={index} className="glass p-4 rounded-lg">
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

function TreePage() {
  const [view, setView] = useState('queue');
  const [blocks, setBlocks] = useState([]);
  const [treeData, setTreeData] = useState(null);
  const [loading, setLoading] = useState(false);
  const apiBase = import.meta.env.VITE_API_BASE;
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
        })
        .catch((err) => console.error('Error fetching blocks:', err))
        .finally(() => setLoading(false));
    } else {
      fetch(`${apiBase}/children_map`)
        .then((res) => res.json())
        .then((data) => {
          if (data.status === 'success' && data.children_map) {
            const childrenMap = new Map();

            data.children_map.forEach((entry) => {
              const children = entry.children.map(([hash, qi]) => ({
                hash,
                qi,
                children: []
              }));
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

            const genesisHash = '000011428da0831df234bd3a0f404f575cf14c9c630f9f6b1c54820a9e2e65ed';
            const root = buildTree(genesisHash, 0);
            setTreeData(root);
          }
          setBlocks([]);
        })
        .catch((err) => console.error('Error fetching children map:', err))
        .finally(() => setLoading(false));
    }
  }, [view]);

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-blue-900 to-gray-900 pt-20 pb-10 px-4">
      <div className="max-w-7xl mx-auto">
        <h1 className="text-4xl font-bold text-white mb-8 text-center animate-fade-in">
          Blockchain Tree Explorer
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
        </div>

        {loading ? (
          <div className="glass p-8 rounded-xl text-center text-white/80">
            <p>Loading...</p>
          </div>
        ) : (
          <div>
            {view === 'queue' && <QueueIndexView blocks={blocks} />}
            {view === 'tree' && <TreeView treeData={treeData} />}
            {view === 'levels' && <LevelView treeData={treeData} />}
          </div>
        )}
      </div>
    </div>
  );
}

export default TreePage;