# CONTEXT.md

## Architecture Overview
* **Relay & Backend**: Rust workspace (`buzz-relay`, `buzz-core`, `buzz-db`, `buzz-auth`, `buzz-pubsub`, `buzz-search`).
* **Desktop Client**: Tauri 2 + React 19 + Vite + Tailwind CSS (`desktop/`).
* **Protocol**: NIP-29 over WebSocket (Nostr protocol) for real-time channels and messaging.

## Deploy Command
* **Desktop App**: `just desktop-tauri-build` / `pnpm tauri:build`
* **Relay Docker**: `docker build -t buzz-relay .`

## Manual Dashboard Config
* N/A — Local development environment initialized via `.env` and Hermit toolchain.

## Knowledge Items (KIs)
* **Desktop Text Sizing**: Use rem-based tokens (`text-base`, `text-xs`, `text-2xs`, `text-3xs`) instead of explicit `px` to maintain webview zoom compatibility.
* **Hermit Environment**: Run commands via `bin/` toolchain wrappers or activate environment before running workspace tools.
* **Format & Lint Gating**: `npx biome check .` and `tsc` must pass before desktop release.

## Pending Tasks
* All desktop linting, unit tests, and production build checks are passing on `dev`.
