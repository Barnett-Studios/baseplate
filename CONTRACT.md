# dotclaude-support — contract

`dotclaude-support` is **substrate**, not a behavioral component. It exposes no runtime
service and makes no swappable-socket promise of its own; its contract is its **public Rust
API** under semver, plus the handful of invariants the components above it rely on.

## Semver

Pre-1.0. The public API may change between **minor** versions (`0.x` → `0.(x+1)`); patch
releases (`0.x.y` → `0.x.(y+1)`) are additive or bug-fix only. Downstream components pin a
compatible minor (`dotclaude-support = "0.1"`). Anything not re-exported from `lib.rs` is
private and carries no guarantee.

## Module invariants

| Module | Invariant relied on by callers |
|---|---|
| `model` | Model identifiers are canonical and stable within a minor; an unknown identifier resolves to a well-defined "unknown" rather than panicking. |
| `trace` | The trace/finding value types are `serde`-(de)serializable and round-trip stable — they cross component boundaries as JSON. |
| `paths` | Every well-known path is resolved through an **env override first**, then a deterministic default. Resolution never touches the filesystem to decide a path (pure), so it is testable without a fixture tree. |
| `registry` | Loading a **missing or malformed** YAML registry is *not* a panic — it returns a typed error the caller can fail-open on. Repo-local entries override global entries by name. |
| `cxpak` | The client tracks the cxpak MCP tool contract (the `op`-parameterized intent tools). A cxpak server that is absent or errors surfaces as a typed error, never a fabricated context bundle. |
| `java_test` | Test-file classification matches the documented suffix rules (`*Test.java` / `*Tests.java` / `src/test/`, `*IT.java`, `*SIT.java` / `src/sit/`, anything under `tests/`) exactly — it is the single source of truth for that split. |
| `patterns` | Shared regexes compile once (`once_cell`) and are `Send + Sync`; callers may hold references across threads. |

## What this crate does not do

- It does not call a language model, spawn the executor, or make network requests of its own
  (the `cxpak` client spawns the cxpak MCP server as a child process — that is its only
  subprocess, and only when constructed).
- It holds no global mutable state beyond lazily-compiled regexes.
- It does not read the environment except through the documented `paths` overrides.

## Stability of the dependency surface

The crate keeps a small, boring dependency set (serde, regex, tokio, rmcp, thiserror). Adding
a heavy or language-model-specific dependency here is a contract-level change — this is the
floor, and the floor stays thin.
