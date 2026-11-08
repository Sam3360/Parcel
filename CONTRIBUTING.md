# Contributing to Parcel

Thanks for helping. Parcel's priorities are, in order: **safety of user data**, **correctness**, **clarity**, then features.

## Ground rules

1. **Never write to, move or delete a user's source files.** Sources are opened read-only. Anything that would change that needs a design discussion first.
2. **Packages are hostile input.** New reader code must validate paths, sizes and counts before trusting them, and come with a test using a crafted archive.
3. **No network access, telemetry or background activity.** Do not add HTTP clients, update checkers or analytics. Links are opened only on explicit user action.
4. **Keep the engine independent.** `parcel-core` and `parcel-workspace` must not depend on Tauri. Tauri commands stay thin adapters.
5. **No raw HTML injection.** `dangerouslySetInnerHTML` is blocked by lint; package content is rendered as React text or inert media.
6. **Tests are part of the change.** Bug fix → regression test. New command → at least one engine test and one UI test where relevant.

## Workflow

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p parcel-core -p parcel-workspace -p parcel-cli
cd app; npm run format:check; npm run lint; npm run typecheck; npm test; npm run build
```

Commit messages: short imperative summary; explain *why* in the body when it is not obvious. Update `CHANGELOG.md` for user-visible changes and `format/SPECIFICATION.md` (+ schema + a test) for any format change. Format changes that are not purely additive need a MAJOR version bump and a migration.

## Reporting security issues

See [docs/SECURITY.md](docs/SECURITY.md). Please do not open public issues for exploitable vulnerabilities.
