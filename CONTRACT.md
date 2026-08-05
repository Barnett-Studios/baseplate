# baseplate — contract

`baseplate` exposes no runtime service and makes no swappable-socket promise of its own; its
contract is its **public Rust API** under semver, plus the handful of invariants the components
above it rely on.

> **This crate is Folding** ([#4](https://github.com/Barnett-Studios/baseplate/issues/4)). Its
> modules are moving to the components that use them. The semver guarantee below is unchanged
> and still honoured for as long as this crate is published — nothing is yanked, and no API is
> removed without a version bump. What changes is where new work on these types happens.

## Semver

Pre-1.0. The public API may change between **minor** versions (`0.x` → `0.(x+1)`); patch
releases (`0.x.y` → `0.x.(y+1)`) are additive or bug-fix only. Downstream components pin a
compatible minor (`baseplate = "0.2"`). Anything not re-exported from `lib.rs` is
private and carries no guarantee.

## Module invariants

| Module | Invariant relied on by callers |
|---|---|
| `model` | **Every** public type in the module is a Promise-Theory verification value type, and all of them are `serde`-(de)serializable with stable wire spellings within a minor — including the cross-boundary result payloads `VerificationResult` and `MethodOutcome`. The invariant is stated over the module rather than a list, because a list is what let the previous version cover five of eleven types. **This module is not a model registry** — it holds no model identifiers, tiers or aliases, and earlier revisions of this contract wrongly promised that it did. |
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
