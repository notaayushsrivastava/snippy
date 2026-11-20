<p align="center">
  <img src="assets/snippy-logo.svg" alt="Snippy logo" width="96" height="96" />
</p>

<h1 align="center">Snippy</h1>

<p align="center">
  A local-first, safety-first cross-platform desktop AI agent for Windows and macOS.
</p>

---

## Overview

Snippy observes a user-selected scope, explains its next step, highlights targets on a transparent click-through overlay, and executes one verified action at a time within an explicit safety policy.

- **Local-first**: Encrypted local session history (AES-256-GCM SQLite) and OS credential storage (Windows Credential Manager / macOS Keychain).
- **Safety-first**: Consequential actions require explicit user approval, secret/password fields always trigger user takeover, and `Esc` / **Stop** halts execution immediately.
- **Multi-provider**: Supports **Gemini 3.8 Live** (with direct screen video streaming), **Gemini 3.5 Flash-Lite**, **OpenAI**, and a local **Demo** mode.

## Workspace Structure

```text
apps/desktop/           Tauri 2 desktop shell & React Command Bar UI
crates/agent-core/      Policy engine, proposal validation, single-step runner
crates/platform-contract/ Typed PlatformAdapter & SecureStorage traits
crates/platform-windows/ Windows UIA, GDI capture, SendInput, Credential Manager
crates/provider-gemini/  Gemini REST + Gemini 3.8 Live WebSocket video adapter
crates/provider-openai/  OpenAI ProviderAdapter
crates/session-store/    In-memory & AES-256-GCM encrypted SQLite store
packages/protocol/       Shared TypeScript protocol types
packages/ui/             3-layer CSS design tokens & accessible UI components
```

## Quick Start (Development)

```bash
# Run the full Tauri 2 desktop app + Vite dev server
bash dev.sh

# Or run the browser UI preview only
bash ui.sh
```

Set `GEMINI_API_KEY` or `OPENAI_API_KEY` in `.env` (for dev) or save your key in **Settings** (`Ctrl+K`) to store it in your OS credential manager.

## Testing & Verification

```bash
# Rust checks & tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Frontend typecheck, tests, and build
corepack pnpm -r --if-present typecheck
corepack pnpm -r --if-present test
corepack pnpm --filter @snippy/desktop build
```

## Releases & Auto-Updates

GitHub Actions workflows in `.github/workflows/` automate CI (`ci.yml`) and signed Windows/macOS releases (`release.yml`) with Tauri's `latest.json` auto-updater.

Before tagging a release (`v*`):
1. Generate a signing keypair: `npx @tauri-apps/cli signer generate -w ~/.tauri/snippy.key`
2. Put the public key in `apps/desktop/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).
3. Add `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to GitHub repository secrets.
