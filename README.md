# baseplate

[![CI](https://github.com/Barnett-Studios/baseplate/actions/workflows/ci.yml/badge.svg)](https://github.com/Barnett-Studios/baseplate/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/baseplate)](https://crates.io/crates/baseplate)
[![docs.rs](https://img.shields.io/docsrs/baseplate)](https://docs.rs/baseplate)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**The shared substrate for agentic-harness tooling — the small, dependency-light crate that
verification, planning, and measurement tools sit on so they don't each re-implement the same floor.**

baseplate is deliberately *not* a tool with a behavioral contract of its own. It is the common floor
underneath the components that verify, plan, and measure an agentic coding loop: the model registry,
the trace/finding types they exchange, root/path resolution, a cxpak MCP client, and Java-test
detection. Depend on it and use only the pieces you need.

> Part of the Barnett Studios agentic-harness toolkit → cxpak · commitward · abproof · cascadr ·
> cordon · planner · **baseplate**

## What's inside

| Module | Responsibility |
|---|---|
| `model` | The model registry — canonical model identifiers and their tiers/aliases. |
| `trace` | The shared trace/finding value types tools exchange (a turn's observed edits). |
| `paths` | Root resolution — `$BASEPLATE_HOME`-anchored, git-tree aware, with no hard-coded home. |
| `registry` | Loading and merging YAML registries (promises, checkpoints) with a stable schema. |
| `cxpak` | A thin [rmcp](https://crates.io/crates/rmcp) client for the cxpak MCP server (child-process transport). |
| `java_test` | Detection of Java test files (unit `*Test.java`, integration `*IT.java`, system `*SIT.java`). |
| `patterns` | Shared regex primitives compiled once, reused across tools. |

## Use

```toml
[dependencies]
baseplate = "0.2"
```

```rust
use baseplate::{model, registry, trace};

// Load a YAML registry (promises/checkpoints) from an explicit path.
let reg = registry::load(std::path::Path::new("registry.yaml"), None)?;
```

Root resolution honours `$BASEPLATE_HOME` (the distribution-safe anchor for a binary installed
outside a git tree), else the git tree the binary lives in, else the current directory — no hard-coded
`$HOME` path. It pulls a small, boring dependency set (serde, regex, tokio, rmcp) and nothing
language-model-specific.

## Stability

Pre-1.0: the surface may change between minor versions. The `cxpak` client tracks the cxpak MCP tool
contract (`op`-parameterized intent tools); a breaking change there is called out in the release notes.
Downstream tools pin a compatible minor.

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
