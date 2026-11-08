# Development guide

## Toolchain

* Rust ≥ 1.85 (developed with 1.91), `rustfmt`, `clippy`
* Node.js ≥ 20
* Windows: Visual Studio Build Tools (C++ workload) + WebView2. Linux (for CI/dev): `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libsoup-3.0-dev libayatana-appindicator3-dev libssl-dev build-essential pkg-config`.

## Everyday commands (PowerShell)

```powershell
# Rust
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p parcel-core -p parcel-workspace -p parcel-cli
cargo check -p parcel-app                # Tauri crate

# Frontend (from app/)
npm run format       # write
npm run format:check
npm run lint
npm run typecheck
npm test
npm run build

# Desktop app
npm run tauri dev
npm run tauri build   # NSIS installer on Windows
```

## Regenerating examples

```powershell
cargo run -p parcel-workspace --example make_examples -- examples
```
`crates/parcel-workspace/tests/examples.rs` fails if a committed example stops verifying.

## Where to add things

| Task | Where |
|---|---|
| New package rule | `parcel-core/src/manifest.rs` or `pathsafe.rs`, plus `format/SPECIFICATION.md`, the JSON Schema and a hostile-input test in `crates/parcel-core/tests/engine.rs` |
| New workspace operation | `parcel-workspace/src/store.rs` / `ops.rs` + test in `tests/workspace.rs` |
| New IPC command | `app/src-tauri/src/commands.rs`, register in `lib.rs`, add to `app/src/lib/api.ts`, `tauriApi.ts` **and** `test/mockApi.ts` |
| New screen | `app/src/pages` or `app/src/features`, with a test using `renderApp` from `src/test/helpers.tsx` |

## Testing notes

Frontend tests run against `createMockApi()`, a stateful in-memory implementation of the `Api` contract. It mirrors backend validation closely but is not the backend; contract drift is limited by typing both sides from the same interface. End-to-end tests of the real Tauri shell are not included.
