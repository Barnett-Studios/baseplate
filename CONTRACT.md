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
| `model` | Every public type in the module is a Promise-Theory verification value type, and every one of them derives `Deserialize` with stable wire spellings within a minor. The ones that leave the process — including the cross-boundary result payloads `VerificationResult` and `MethodOutcome` — also derive `Serialize` and round-trip. Two do not: `PromiseSpec` and `Requires` are registry *input* only, read from YAML and never written back, so they are deserialize-only by design. **This module is not a model registry** — it holds no model identifiers, tiers or aliases, and earlier revisions of this contract wrongly promised that it did. |
| `trace` | The trace/finding value types are `serde`-(de)serializable and round-trip stable — they cross component boundaries as JSON. |
| `paths` | Root resolution is **exe-first, not env-first**: `repo_root(current_exe())`, then `$BASEPLATE_HOME` if it names an existing directory, then `repo_root(current_dir())`, then `current_dir()`, then `"."`. Exe-first is the distribution-safe order — an installed binary must not resolve to whatever repo the caller happens to be sitting in — and the **consequence is that `$BASEPLATE_HOME` is ignored for any binary that lives inside a git tree**, which is every `cargo run`/`cargo test` and this assembly's own development path. Resolution **does** consult the filesystem to decide: `repo_root` stats `.git` at each ancestor, and the env candidate is accepted only `if is_dir()`. It is deterministic given the tree, not pure. |
| `registry` | Loading a **missing or malformed** YAML registry is *not* a panic — it returns a typed error the caller can fail-open on. Repo-local entries override global entries by name. |
| `cxpak` | The client tracks the cxpak MCP tool contract (the `op`-parameterized intent tools). A cxpak server that is absent or errors surfaces as a typed error, never a fabricated context bundle. A call made before cxpak's background index is warm is **retried**, not returned as junk — the still-indexing response is not JSON, so without the retry it is indistinguishable from a parse failure. The retry budget is 25s, overridable with `CXPAK_INDEX_WARM_BUDGET_MS`; on expiry the call returns `None` and the caller skips (fail-open, unchanged). |
| `java_test` | Test-file classification matches the documented suffix rules (`*Test.java` / `*Tests.java` / `src/test/`, `*IT.java`, `*SIT.java` / `src/sit/`, anything under `tests/`) exactly — it is the single source of truth for that split. |
| `patterns` | Shared regexes compile once (`once_cell`) and are `Send + Sync`; callers may hold references across threads. |

## What this crate does not do

- It does not call a language model, spawn the executor, or make network requests of its own
  (the `cxpak` client spawns the cxpak MCP server as a child process — that is its only
  subprocess, and only when constructed).
- It holds no global mutable state beyond lazily-compiled regexes.
- It does not read the environment except through the documented `paths` overrides and
  `CXPAK_INDEX_WARM_BUDGET_MS`. Both are read at the point of use and never cached, and an
  absent, blank or unparseable value falls back to the compiled default rather than to zero
  — a budget of zero would turn every cold-index call into an immediate skip.

## Stability of the dependency surface

The crate keeps a small, boring dependency set (serde, regex, tokio, rmcp, thiserror). Adding
a heavy or language-model-specific dependency here is a contract-level change — two crates
compile this one in, and it stays thin for their sake.
