# baseplate

[![CI](https://github.com/Barnett-Studios/baseplate/actions/workflows/ci.yml/badge.svg)](https://github.com/Barnett-Studios/baseplate/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/baseplate)](https://crates.io/crates/baseplate)
[![docs.rs](https://img.shields.io/docsrs/baseplate)](https://docs.rs/baseplate)
[![ghcr.io](https://img.shields.io/badge/ghcr.io-baseplate-blue?logo=docker)](https://github.com/Barnett-Studios/baseplate/pkgs/container/baseplate)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**The shared substrate for agentic-harness tooling — the small, dependency-light crate that
verification, planning, and measurement tools sit on so they don't each re-implement the same floor.**

baseplate is deliberately *not* a tool with a behavioral contract of its own. It is the common floor
underneath the components that verify, plan, and measure an agentic coding loop: the model registry,
the trace/finding types they exchange, root/path resolution, a cxpak MCP client, and Java-test
detection. Depend on it and use only the pieces you need.

> Part of the Barnett Studios agentic-harness toolkit → cxpak · commitward · abproof · cascadr ·
> cordon · slicr · **baseplate**

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

## The query CLI — the invocable ops as a one-shot container

baseplate's dominant value is the shared **types** other crates compile against — a role that has
no runtime surface and stays a compile-time crate dependency. But a subset of its modules have
genuinely-invocable ops, and those ship as a self-contained CLI (a `cli` feature) packaged as a
container image, so any harness can use them without linking the crate. Each op reads a JSON
request on stdin and writes an [ADR-0052](https://github.com/Barnett-Studios/baseplate) envelope
(`{schema_version, status, body}`) on stdout, network-free:

```console
$ echo '{"paths":["src/test/java/x/FooTest.java","src/main/java/x/Foo.java"]}' \
    | docker run --rm -i --network none ghcr.io/barnett-studios/baseplate java-test analyze
{"schema_version":"1","status":"ok","body":{"results":[{"path":"src/test/java/x/FooTest.java","is_java_test":true},...]}}
```

| Op | Request | Answers |
|---|---|---|
| `java-test analyze` | `{paths: [String]}` | which paths are Java test files |
| `patterns match` | `{pattern, content, case_insensitive?}` | does a regex compile, and where it matches |
| `registry load` | `{registry_yaml, overrides_yaml?}` | parse + summarize a promise registry (valid? version, promises) |

An uncompilable pattern or invalid registry is a *result* (`compiles: false` / `valid: false`),
not a failure; only a malformed **request** yields `status: "error"` + a non-zero exit, so a
consumer falls back to its in-process path rather than trusting an empty result.

**Honest scope.** `model`/`trace` (shared types) and the `cxpak` client (needs a live MCP server)
have no mount-free surface and are intentionally not exposed here — this image is baseplate's
*consumable* face, not its primary role, which stays a linked crate. The image is **standalone**:
no other component derives `FROM` it. Build the binary from source with `cargo build --release
--features cli`; it's also on the [Homebrew tap](https://github.com/Barnett-Studios/homebrew-tap)
(`brew install barnett-studios/tap/baseplate`).

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
[slicr](https://github.com/Barnett-Studios/slicr).
