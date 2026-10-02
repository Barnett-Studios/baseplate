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

**A change to observable BEHAVIOUR of a published surface is a minor, even when no type
signature moves.** The compiler is not the test; a consumer asserting the old output is. This
is a rule rather than a case-by-case judgement because the alternative was measured and is
worse (baseplate#15): a patch is compatible with `"0.2"`, so every consumer picks it up on its
next `cargo update` **with no commit of its own**, and CI goes red in a repo whose diff does
not contain the cause. Nothing warns; the lockfile moved. That is the expensive kind of break,
and a minor is what stops it — nothing moves until a consumer opts in, which is the honest
signal for a behaviour change.

**A minor here is a lockstep release, not a solo one.** `attestr` re-exports baseplate types,
and the assembly names them on both sides of that seam, so a consumer on `attestr = "0.4"`
(which pins `baseplate = "0.2"`) plus `baseplate = "0.3"` links two baseplates and stops
compiling:

```
note: there are multiple different versions of crate `baseplate` in the dependency graph
$ cargo tree -d
baseplate v0.2.1
└── attestr v0.4.1
baseplate v0.3.0
```

It fails loudly at compile time rather than silently wiring to the older type, which is why
this is a sequencing obligation and not a correctness hazard. The order:

1. `baseplate` publishes `0.3.0` to crates.io.
2. `attestr` moves to `baseplate = "0.3"` and releases (a re-exported dependency's minor is
   attestr's own minor, per its CONTRACT).
3. The consuming assembly bumps both pins in ONE commit, and fixes the tests that go red in
   that same commit — those failures are the *point* of the bump, so splitting them means
   knowingly landing a red commit.

The order is forced, not preferred: `cargo publish` resolves every dependency against the
registry before it will upload, so an `attestr` release can only carry `baseplate = "0.3"`
once that version is actually **on crates.io** — not once it is merged, tagged, or version-
bumped here. It is not a risky ordering; it is the only one that can execute. attestr's own
`Cargo.toml` carries the same constraint (*"baseplate + cascadr must be on crates.io before
attestr"*), so this records the reason rather than restating the rule.

The distinction between bumped and published is the part that bites. On 2026-09-05 this repo
committed `chore(release): baseplate 0.3.0` while the registry still served only `0.2.0` and
`0.2.1` — a normal and correct intermediate state, and precisely the window in which an
attestr release would fail to resolve. Read the registry, not `Cargo.toml`, before starting
step 2.

**The next release out of this repo is `0.3.0`.** Recorded here so it is not re-litigated. It
carries three things, each of which independently forces a minor: baseplate#12 and #14, merged
and unreleased since `v0.2.1` and both behaviour changes on published surface; and a fifth
`ReviewParser` variant (baseplate#30, for attestr#31 under the founder decision of
2026-09-02), which is breaking for any exhaustive matcher.

Two things follow, and both have bitten before:

- **`Cargo.toml` will read `0.2.1` while a breaking change sits on `main`.** That is
  deliberate — cutting the release is not the Coder's — and it means the version cannot be
  inferred from the file at that moment. It is written here instead.
- **The bump must be committed BEFORE the tag.** The ghost check's half 4 asserts that a tag's
  tree declares the version the tag names, so a tag cut over an unbumped tree fails that half
  and publishes the crate as `0.2.1`.

`0.3.0` is now on crates.io; the three items above are history, kept rather than deleted because
the ordering lesson applies again below.

**The next release out of this repo is `0.4.0`.** Recorded here so it is not re-litigated. It
carries `ReviewDecision::independence: Independence` (attestr#1 — the field a glue consumer uses
to record whether a reviewer ran on a harness distinct from the turn's author; this crate does
not decide the value, only gives it somewhere honest to live). `#[serde(default)]` makes a
record persisted before this field existed deserialize cleanly to `Independence::Unknown`, so
this is **not** a wire-format break — but it **is** a struct-literal break: any `ReviewDecision {
..fields.. }` literal elsewhere in the family needs the new field added, which is why this is a
minor and not a patch despite the wire compatibility.

`Unknown` is the fail-open default, not evidence of anything — it is what "never checked" and
"checked and the identity was missing" both deserialize to, on purpose, since a reader cannot
and must not try to tell them apart. **A consumer must treat `Unknown` as "independence was not
shown" and must never gate on `== SameHarness` alone**; a gate written as "proceed unless
`SameHarness`" treats "nobody checked" as a pass.

Known re-pins, so the lockstep isn't rediscovered one broken build at a time:

| repo | file | current pin |
|---|---|---|
| `attestr` | `Cargo.toml:50` | `baseplate = "0.3"` |
| `conductr` (`conductr-core`) | `crates/conductr-core/Cargo.toml:26` | `baseplate = "0.3"` |
| `dotgithub` (`qa/release-retest`) | `qa/release-retest/Cargo.toml:11` | `baseplate = "0.2"` — already behind `0.3.0`, independent of this release |

Two repos have an actual `ReviewDecision` struct literal that needs the new field added once
they move to `baseplate = "0.4"` — not just `attestr`:

| repo | literal sites |
|---|---|
| `attestr` | `src/reviewer.rs:480, :501, :511` |
| `conductr` (`conductr-core`) | `crates/conductr-core/src/engine.rs:467, :525` (via the re-exported `baseplate::model`, as `crate::model::ReviewDecision`) |

`dotgithub` (`qa/release-retest`) consumes the crate without constructing that type and only
needs the pin bumped to keep resolving a current `baseplate`.

Also: a serialized `ReviewDecision` on `0.4` always carries an `"independence"` key that `0.3`
never did. Not a wire break for a *reader* — neither this crate nor any known consumer derives
`#[serde(deny_unknown_fields)]` on it, so an extra key is additive — but a writer snapshotting
fixtures byte-for-byte (a golden-file test comparing serialized JSON) will see new output.

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
