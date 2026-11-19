#!/usr/bin/env bash
set -euo pipefail

if command -v pnpm >/dev/null 2>&1; then
  PNPM="pnpm"
else
  PNPM="corepack pnpm"
fi

if [ ! -d "node_modules" ]; then
  echo "Installing workspace dependencies..."
  $PNPM install
fi

if [ "${1:-}" = "--web" ]; then
  echo "Starting Snippy web UI preview..."
  exec $PNPM --filter @snippy/desktop dev
fi

echo "Starting Vite dev server and Tauri desktop app..."
$PNPM --filter @snippy/desktop dev &
VITE_PID=$!
trap 'kill "$VITE_PID" 2>/dev/null || true' EXIT

sleep 2
cargo run -p snippy-desktop
