#!/bin/bash

# Kill old session if it exists
tmux kill-session -t tct 2>/dev/null

# === BACKEND NODES ===
tmux new-session -d -s tct "cargo run --bin TreeChainTheorey"
sleep 0.5

tmux split-window -h "ALIGN=2 HTTP_PORT=3002 P2P_PORT=5002 cargo run --bin TreeChainTheorey"
sleep 0.5

tmux split-window -v "ALIGN=3 HTTP_PORT=3003 P2P_PORT=5003 cargo run --bin TreeChainTheorey"
sleep 0.5

tmux select-pane -t 0
tmux split-window -v "ALIGN=1 HTTP_PORT=3004 P2P_PORT=5004 cargo run --bin TreeChainTheorey"
sleep 0.5

tmux select-layout tiled
sleep 0.5

# === FRONTEND NODES (Window 2) ===
tmux new-window -t tct:1 -n "frontend1" "cd frontend && VITE_API_BASE=http://localhost:3001 npm run dev"
sleep 0.5

tmux split-window -h "cd frontend && VITE_API_BASE=http://localhost:3002 npm run dev"
sleep 0.5

tmux split-window -v "cd frontend && VITE_API_BASE=http://localhost:3003 npm run dev"
sleep 0.5

tmux select-pane -t 0
tmux split-window -v "cd frontend && VITE_API_BASE=http://localhost:3004 npm run dev"
sleep 0.5

tmux select-layout tiled
sleep 0.5

# === EXTRA BACKEND NODES (Window 3) ===
tmux new-window -t tct:2 -n "extra_nodes"
tmux send-keys -t tct:2 "ALIGN=2 HTTP_PORT=3005 P2P_PORT=5005 cargo run --bin TreeChainTheorey" C-m
sleep 0.5

tmux split-window -h "ALIGN=3 HTTP_PORT=3006 P2P_PORT=5006 cargo run --bin TreeChainTheorey"
sleep 0.5

tmux select-layout even-horizontal
sleep 0.5

# === EXTRA FRONTEND NODES (Window 4) ===
tmux new-window -t tct:3 -n "extra_front"
tmux send-keys -t tct:3 "cd frontend && VITE_API_BASE=http://localhost:3005 npm run dev" C-m
sleep 0.5

tmux split-window -h "cd frontend && VITE_API_BASE=http://localhost:3006 npm run dev"
sleep 0.5

tmux select-layout even-horizontal

# === ATTACH ===
tmux attach -t tct
