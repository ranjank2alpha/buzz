// Fork override: upstream sets the frontend cap at 1200, but this fork carries
// larger monolithic hooks/components (its own features plus upstream's growth on
// every catch-up), so we hold it at 1300 — the ceiling adopted in 0.5.20-1.
// `allowedLineCount` still pins any file already over its baseline, so this only
// widens the headroom for genuinely new growth. Rust keeps upstream's 1500.
const DESKTOP_FRONTEND_MAX_LINES = 1300;
const DESKTOP_RUST_MAX_LINES = 1500;

export const rules = [
  {
    root: "src-tauri/src",
    extensions: new Set([".rs"]),
    maxLines: DESKTOP_RUST_MAX_LINES,
  },
  // Workspace member crates. Without this the ratchet's only Rust root is
  // `src-tauri/src`, and a crate under `src-tauri/crates/` is born outside the
  // repo's one size discipline -- silently, since the check still exits 0.
  {
    root: "src-tauri/crates",
    extensions: new Set([".rs"]),
    maxLines: DESKTOP_RUST_MAX_LINES,
  },
  {
    root: "src/app",
    extensions: new Set([".ts", ".tsx"]),
    maxLines: DESKTOP_FRONTEND_MAX_LINES,
  },
  {
    root: "src/features",
    extensions: new Set([".ts", ".tsx"]),
    maxLines: DESKTOP_FRONTEND_MAX_LINES,
  },
  {
    root: "src/shared/api",
    extensions: new Set([".ts", ".tsx"]),
    maxLines: DESKTOP_FRONTEND_MAX_LINES,
  },
  {
    root: "src/shared/context",
    extensions: new Set([".ts", ".tsx"]),
    maxLines: DESKTOP_FRONTEND_MAX_LINES,
  },
  {
    root: "src/shared/lib",
    extensions: new Set([".ts", ".tsx"]),
    maxLines: DESKTOP_FRONTEND_MAX_LINES,
  },
  {
    root: "src/shared/ui",
    extensions: new Set([".ts", ".tsx"]),
    maxLines: DESKTOP_FRONTEND_MAX_LINES,
  },
  {
    root: "src/shared/styles",
    extensions: new Set([".css"]),
    maxLines: DESKTOP_FRONTEND_MAX_LINES,
  },
];
