# dotclaude-support

[![CI](https://github.com/Barnett-Studios/dotclaude-support/actions/workflows/ci.yml/badge.svg)](https://github.com/Barnett-Studios/dotclaude-support/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/dotclaude-support)](https://crates.io/crates/dotclaude-support)
[![docs.rs](https://img.shields.io/docsrs/dotclaude-support)](https://docs.rs/dotclaude-support)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**The shared support layer for the Barnett Studios agentic-harness toolkit — the small,
dependency-light substrate that several components sit on so they don't each re-implement it.**

This crate is deliberately *not* a component with a behavioral contract of its own. It is the
common floor underneath the components that verify, plan, and measure an agentic coding loop:
the model registry, the trace/finding types they exchange, path resolution, the cxpak MCP
client, and Java-test detection. Extracting it is what breaks the dependency cycle that would
otherwise couple the Verifier to the rest of the harness core.

> Part of the Barnett Studios agentic-harness toolkit → cxpak · commitward · abproof · cascadr ·
> cordon · planner · **dotclaude-support**

## What's inside

| Module | Responsibility |
|---|---|
| `model` | The model registry — canonical model identifiers and their tiers/aliases. |
| `trace` | The shared trace/finding value types components exchange (a turn's observed edits). |
| `paths` | Deterministic path resolution for the harness's well-known directories (env-overridable). |
| `registry` | Loading and merging YAML registries (promises, checkpoints) with a stable schema. |
| `cxpak` | A thin [rmcp](https://crates.io/crates/rmcp) client for the cxpak MCP server (child-process transport). |
| `java_test` | Detection of Java test files (unit `*Test.java`, integration `*IT.java`, system `*SIT.java`). |
| `patterns` | Shared regex primitives compiled once, reused across components. |

## Use

```toml
[dependencies]
dotclaude-support = "0.1"
```

```rust
use dotclaude_support::{model, paths, registry, trace};

// Resolve a well-known harness directory (honours the env override).
let promises = registry::load(paths::config_dir().join("promise/registry.yaml"))?;
```

Each module is independent — depend on the crate and use only the pieces you need. It pulls a
small, boring dependency set (serde, regex, tokio, rmcp) and nothing language-model-specific.

## Stability

Pre-1.0: the surface may change between minor versions. The `cxpak` client tracks the cxpak
MCP tool contract (`op`-parameterized intent tools); a breaking change there is called out in
the release notes. Downstream components pin a compatible minor.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
Unless you explicitly state otherwise, any contribution you intentionally submit for
inclusion in the work shall be dual-licensed as above, without any additional terms.

---

Built by [Barnett Studios](https://barnett-studios.com/) — the substrate under
[cxpak](https://github.com/Barnett-Studios/cxpak) ·
[commitward](https://github.com/Barnett-Studios/commitward) ·
[cascadr](https://github.com/Barnett-Studios/cascadr) ·
[abproof](https://github.com/Barnett-Studios/abproof) ·
[cordon](https://github.com/Barnett-Studios/cordon) ·
[planner](https://github.com/Barnett-Studios/planner).
