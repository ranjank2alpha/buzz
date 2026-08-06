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
* **Windows Notifications**: For Tauri v2 notifications to appear in Windows OS Notification settings, explicitly call `app.set_app_user_model_id("xyz.block.buzz.app")` in the Tauri setup hook, and the app must be formally installed (shortcut created) via the generated NSIS installer.
* **Google SSO Flow**: Ensure Google Auth (`start_google_workspace_login`) handles UI state and error reporting seamlessly to bypass manual key import flows when using SSO.
* **Google OAuth Desktop Client Secret**: Google strictly requires the `client_secret` parameter in the token exchange for "Desktop app" OAuth clients (even when PKCE is fully implemented). To avoid hardcoding it and triggering secret scanning, the secret is injected at compile time via `build.rs` using the `BUZZ_BUILD_GOOGLE_CLIENT_SECRET` env var and read via `option_env!`. CI workflows pass this in via GitHub Secrets.
* **Stale Cargo Cache in CI**: The `desktop-release-cache-key.py` script hashes only `Cargo.toml`, `Cargo.lock`, and `rust-toolchain.toml` — NOT `.rs` source files. If only Rust source changes (no dependency changes), the cache key stays identical and Cargo reuses old compiled objects, silently ignoring source edits. Fix: add `cargo clean -p buzz-desktop --release --target "$TARGET"` before `pnpm tauri build` in the workflow.
* **Canary Installer Naming**: Windows Canary installer is named `Buzz <version>-fork-ddmm-hhmm.exe` via the `productName` field in `tauri.canary.conf.json` (generated dynamically in `.github/workflows/windows-canary.yml`). This ensures every installed build is visually identifiable.

## Coding Guidance (Agent Contract)

Applies to every agent and human touching this repo. These are rules of construction, not a checklist of blessed values — never satisfy one by pasting a literal into source.

### 1. Configuration & Secrets
* **No literals for anything environment-dependent.** Credentials, endpoints, ports, bucket names, model IDs, feature toggles: resolve from env at runtime, or via `option_env!` at compile time when the value must be baked into a shipped binary. Source holds the *name* of the variable, never the value.
* **Every new variable is declared in `.env.example`** in the same commit that reads it, with a comment on what it does and whether it is required.
* **Missing required config fails loudly at startup**, not lazily at first use. A binary that boots and then dies mid-OAuth is worse than one that refuses to boot.
* **Never disable, bypass, or annotate around a secret-scanning block.** A scanner hit means the value belongs in CI secrets, not that the scanner is wrong. If a value genuinely cannot be externalized, it is an accepted risk requiring a written entry under "Accepted Risks" below — not a silent bypass.
* **Never log, serialize, or return a secret**, including in error strings and `Debug` impls. Redact tokens, keys, and authorization codes at the boundary.

### 2. Cryptographic & Identity Material
* **Key material must come from a CSPRNG**, or from a KDF whose input includes at least 128 bits of attacker-unknown entropy. A hash over a public identifier (email, OAuth `sub`, user ID, device ID) is not a secret regardless of how the salt is chosen.
* **Constants committed to the repo are public.** Never treat a hardcoded salt, pepper, or seed as a security boundary.
* **Private keys live in the OS keychain / secure enclave**, never in plaintext files, app state, or anything crossing the Tauri IPC boundary unless the user explicitly initiated an export.
* **Deriving identity from an SSO subject is not a substitute for key storage.** If cross-device recovery is needed, escrow an encrypted key to a k2alpha-controlled service; do not make the key recomputable from public inputs.

### 3. OAuth & Authentication Flows
* Send and verify a random `state` parameter on every authorization request. PKCE protects code exchange; it does not authenticate the callback.
* Bind the loopback callback to `127.0.0.1` (never `0.0.0.0`), reject callbacks whose `state` does not match, and treat any unexpected request on the callback port as hostile.
* Validate every claim you depend on (`hd`, `email`, `email_verified`, `aud`, `exp`) explicitly. Do not rely on request-time hints like `hd=` in the auth URL — those are UI suggestions, not enforcement.
* Only skip JWT signature verification when the token was received directly from the issuer's token endpoint over TLS in the same function. If a token-parsing helper could ever be handed a token from another source, it verifies the signature or it does not exist.

### 4. External Calls & Failure Handling
* **Every outbound call sets an explicit connect timeout and total timeout.** No unbounded waits, ever. Reuse a configured client; do not construct a default client per call site.
* **Retries are bounded, jittered, and only for idempotent or explicitly retry-safe operations.** Never retry a token exchange or any single-use code redemption.
* **Distinguish failure classes** — network, 4xx, 5xx, malformed payload — and surface them as typed errors. `Result<_, String>` is acceptable only at the Tauri command boundary, and only after the typed error has been logged.
* **Degrade, don't hang.** A dependency being down produces an actionable user-facing message within the timeout window.

### 5. Resource Lifecycle
* Anything spawned, bound, or locked has exactly one guaranteed teardown path that runs on **all** exits — success, error, timeout, and early `?` return. Prefer RAII guards over manual cleanup calls placed after the happy path.
* No `unwrap()` / `expect()` on anything reachable from user input, IPC, or the network. Poisoned-lock recovery is explicit.
* Long-lived tasks are cancellable and observable; a leaked listener or task is a defect even when it is invisible.

### 6. Observability
* Use `tracing` with structured fields, not `println!`. Instrument boundaries: IPC entry, external call, auth decision, error return.
* Log the decision *and* its inputs (redacted) at the point where the code takes a branch a support engineer would later need to explain.
* Error paths log at `warn`/`error` with enough context to diagnose without a reproduction. Silent `Err` returns are not acceptable.

### 7. Fork Discipline (k2alpha ← block/sprout)
* **Keep k2alpha-specific deltas small, isolated, and clearly marked** so upstream merges stay mechanical. Prefer a dedicated module or config surface over edits scattered across upstream files.
* **Never restructure upstream code opportunistically.** Every diff against upstream is a future merge conflict; each one must be justified by a k2alpha requirement.
* **Forking is not a justification for a weaker practice.** If a control is hard to implement because of the fork, say so explicitly and record it under Accepted Risks — do not silently lower the bar.

### 8. Change Discipline
* Touch only what the task requires; no drive-by refactors, reformatting, or "while I'm here" cleanup.
* Behavioral changes ship with a test that fails before the change and passes after. Deterministic logic gets unit tests; external dependencies are mocked.
* `cargo clippy`, `cargo fmt`, `npx biome check .`, and `tsc` pass before any release build.
* Repo root stays clean: no installers, binaries, scratch scripts, logs, or dumped JSON. Working files go under an ignored directory.

### Accepted Risks (k2alpha, reviewed — not patterns to copy)
* **Hardcoded Google domain (`k2alpha.ai`)**: intentional. This fork serves one company; the domain is a product constraint, not configuration. *Not* an exception to §1 for any other value.

Adding to this list requires a stated mitigation and a trigger condition for removing it. An entry without both is a bug, not an accepted risk.

## Pending Tasks
* Await completion of Windows Canary build (Run #5, with cache bust + client_secret fix) to verify Google login works end-to-end.
* Rotate the exposed Google OAuth client secret and update the CI secret (`BUZZ_GOOGLE_CLIENT_SECRET`).
