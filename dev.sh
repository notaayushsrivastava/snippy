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

echo "Starting Snippy desktop UI in dev mode..."
$PNPM --filter @snippy/desktop dev
