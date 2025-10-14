#!/bin/bash
# === Start mining on all backends ===

echo "🚀 Triggering /start_mining on all backend nodes..."

for port in 3001 3002 3003 3004 3005 3006; do
  echo "→ Calling http://localhost:$port/start_mining"
  curl -s "http://localhost:$port/start_mining" > /dev/null
  if [ $? -eq 0 ]; then
    echo "✅ Node on port $port started mining"
  else
    echo "⚠️ Node on port $port failed or not reachable"
  fi
done