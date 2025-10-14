#!/bin/bash
SESSION="tct"

# Kill old tmux session if exists
tmux kill-session -t $SESSION 2>/dev/null

# === Step 1: Start Ports Server ===
echo "🚀 Starting ports_server..."
tmux new-session -d -s $SESSION "cargo run --bin ports_server"
sleep 2

# === Step 2: Start Backends (2 per window) ===
echo "🌳 Starting TreeChainTheorey backend nodes..."

# Window 1
tmux new-window -t $SESSION:1 -n "backends_1"
tmux send-keys -t $SESSION:1 "cargo run --bin TreeChainTheorey" C-m
sleep 0.5
tmux split-window -h -t $SESSION:1 "ALIGN=2 HTTP_PORT=3002 P2P_PORT=5002 cargo run --bin TreeChainTheorey"
sleep 0.5
tmux select-layout -t $SESSION:1 even-horizontal

# Window 2
tmux new-window -t $SESSION:2 -n "backends_2"
tmux send-keys -t $SESSION:2 "ALIGN=3 HTTP_PORT=3003 P2P_PORT=5003 cargo run --bin TreeChainTheorey" C-m
sleep 0.5
tmux split-window -h -t $SESSION:2 "ALIGN=1 HTTP_PORT=3004 P2P_PORT=5004 cargo run --bin TreeChainTheorey"
sleep 0.5
tmux select-layout -t $SESSION:2 even-horizontal

# Window 3
tmux new-window -t $SESSION:3 -n "backends_3"
tmux send-keys -t $SESSION:3 "ALIGN=2 HTTP_PORT=3005 P2P_PORT=5005 cargo run --bin TreeChainTheorey" C-m
sleep 0.5
tmux split-window -h -t $SESSION:3 "ALIGN=3 HTTP_PORT=3006 P2P_PORT=5006 cargo run --bin TreeChainTheorey"
sleep 0.5
tmux select-layout -t $SESSION:3 even-horizontal

# Give backends a few seconds to fully boot
echo "⏳ Waiting for backends to be ready..."
sleep 3

# === Step 3: Start Mining on All Backends ===
echo "⚒️  Triggering /start_mining on all backend nodes..."
for port in 3001 3002 3003 3004 3005 3006; do
  echo "→ Calling http://localhost:$port/start_mining"
  curl -s "http://localhost:$port/start_mining" > /dev/null
  if [ $? -eq 0 ]; then
    echo "✅ Node on port $port started mining"
  else
    echo "⚠️ Node on port $port failed or not reachable"
  fi
done

# === Step 4: Start Frontends (2 per window) ===
echo "🌐 Starting frontend nodes..."

# Window 4
tmux new-window -t $SESSION:4 -n "frontends_1"
tmux send-keys -t $SESSION:4 "cd frontend && VITE_API_BASE=http://localhost:3001 npm run dev -- --port 5173" C-m
tmux split-window -h -t $SESSION:4 "cd frontend && VITE_API_BASE=http://localhost:3002 npm run dev -- --port 5174"
tmux select-layout -t $SESSION:4 even-horizontal

# Window 5
tmux new-window -t $SESSION:5 -n "frontends_2"
tmux send-keys -t $SESSION:5 "cd frontend && VITE_API_BASE=http://localhost:3003 npm run dev -- --port 5175" C-m
tmux split-window -h -t $SESSION:5 "cd frontend && VITE_API_BASE=http://localhost:3004 npm run dev -- --port 5176"
tmux select-layout -t $SESSION:5 even-horizontal

# Window 6
tmux new-window -t $SESSION:6 -n "frontends_3"
tmux send-keys -t $SESSION:6 "cd frontend && VITE_API_BASE=http://localhost:3005 npm run dev -- --port 5177" C-m
tmux split-window -h -t $SESSION:6 "cd frontend && VITE_API_BASE=http://localhost:3006 npm run dev -- --port 5178"
tmux select-layout -t $SESSION:6 even-horizontal

# === Step 5: Auto Open Browser Tabs ===
echo "🌍 Opening all frontend URLs in browser..."
sleep 2
for port in 5173 5174 5175 5176 5177 5178; do
  open "http://localhost:$port"
done

# === Step 6: Attach ===
echo "🎯 All systems running! Attaching to tmux..."
tmux attach -t $SESSION
