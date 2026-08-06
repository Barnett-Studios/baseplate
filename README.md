# baseplate

[![CI](https://github.com/Barnett-Studios/baseplate/actions/workflows/ci.yml/badge.svg)](https://github.com/Barnett-Studios/baseplate/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/baseplate)](https://crates.io/crates/baseplate)
[![docs.rs](https://img.shields.io/docsrs/baseplate)](https://docs.rs/baseplate)
[![ghcr.io](https://img.shields.io/badge/ghcr.io-baseplate-blue?logo=docker)](https://github.com/Barnett-Studios/baseplate/pkgs/container/baseplate)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**Status: Folding.** See the [component map](https://github.com/Barnett-Studios).

baseplate is the one repo in the family with no architectural position of its own, and it is being
dissolved into the components that actually use its contents — tracked in
[#4](https://github.com/Barnett-Studios/baseplate/issues/4). It calls itself a substrate, but a
115-byte library surface with two consumers whose largest file is a cxpak MCP client is a grab-bag,
not a floor. A client belongs with its server; verification types belong with the verifier.

**Nothing is archived and no published version is yanked.** Every release on crates.io stays
available and anything depending on one keeps working. Destinations are still being decided
per-module; this README is updated with the map once the moves land, rather than announcing
them in advance.

**A small, dependency-light crate of shared types and helpers, with two consumers.** Publicly,
[attestr](https://github.com/Barnett-Studios/attestr) — the Verifier — pins `baseplate = "0.2"`;
privately, dotclaude-core does the same. Nothing else in the family links it.

baseplate is deliberately *not* a tool with a behavioral contract of its own, and it is not the
floor under the toolkit either — that is the claim [#4](https://github.com/Barnett-Studios/baseplate/issues/4)
retracts. What is actually here: Promise-Theory verification value types, the trace/finding types
that go with them, root/path resolution, a cxpak MCP client, YAML registry loading, and Java-test
detection. Depend on it and use only the pieces you need.

> Part of the Barnett Studios agentic-harness toolkit → cxpak · commitward · abproof · cascadr ·
> cordon · slicr · corpus · attestr · **baseplate**

## What's inside

| Module | Responsibility |
|---|---|
| `model` | Promise-Theory verification value types — `Observation` (Kept/Broken/Partial/Skipped), `Confidence`, `PromiseType`, `PromiseSpec`, `ReviewDecision`. **Not** a model registry: there are no model identifiers, tiers or aliases here. The module name is a misnomer, and it is left in place rather than renamed because the module is moving out under [#4](https://github.com/Barnett-Studios/baseplate/issues/4). |
| `trace` | The shared trace/finding value types tools exchange (a turn's observed edits). |
| `paths` | Root resolution — `$BASEPLATE_HOME`-anchored, git-tree aware, with no hard-coded home. |
| `registry` | Loading and merging YAML **promise/checkpoint** registries with a stable schema. Unrelated to `model`. |
| `cxpak` | A thin [rmcp](https://crates.io/crates/rmcp) client for the cxpak MCP server (child-process transport). |
| `java_test` | Detection of Java test files (unit `*Test.java`, integration `*IT.java`, system `*SIT.java`). |
| `patterns` | Shared regex primitives compiled once, reused across tools. |

## Use

```toml
[dependencies]
baseplate = "0.2"
```

While it is Folding, `baseplate = "0.2"` keeps working exactly as it does today. No release is
yanked and no API is removed without a version bump.

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

Built by [Barnett Studios](https://barnett-studios.com/) — part of the agentic-harness toolkit.
The one public crate that depends on baseplate is
[attestr](https://github.com/Barnett-Studios/attestr): `baseplate = "0.2"` on every release from
0.2.0 to 0.4.1, and 0.1.0 on this crate's former name, `dotclaude-support`. The other consumer is
private. Every other published version of cxpak, commitward, cascadr and abproof declares no
baseplate dependency (checked against the crates.io index, all versions, not just the latest), and
cordon, slicr and corpus are not Rust crates. They are siblings in the toolkit; none of them links
this one. A footer claiming otherwise is what
[#4](https://github.com/Barnett-Studios/baseplate/issues/4) opened over.
