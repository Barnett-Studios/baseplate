//! `baseplate` — the substrate's genuinely-invocable query ops as one-shot CLIs (ADR-0054).
//!
//! baseplate's dominant value is the shared *types* other crates compile against; that has no
//! runtime surface and is not — cannot be — containerized. This CLI exposes only the small subset
//! of ops that have an honest out-of-process surface. Each reads a JSON request on stdin and
//! writes an ADR-0052 `{schema_version, status, body}` envelope on stdout:
//!
//! ```text
//!   baseplate java-test analyze   {paths: [String]}                     -> per-path is_java_test
//!   baseplate patterns  match     {pattern, content, case_insensitive?} -> compiles + matches
//!   baseplate registry  load      {registry_yaml, overrides_yaml?}      -> parsed registry summary
//! ```
//!
//! `paths resolve` from ADR-0054 is intentionally absent: `repo_root`/`framework_root` resolve
//! against the *container's* filesystem and environment, so with no mount they cannot see the
//! caller's tree — there is no honest mount-free surface. That finding is recorded in ADR-0054
//! rather than shipped as theater. Everything here is self-contained: no network, no filesystem
//! reads beyond a self-cleaning temp file for the (path-based) registry loader — safe under
//! `docker run --network none`.

use std::io::Read;
use std::path::PathBuf;

use baseplate::{java_test, patterns, registry};
use serde::Deserialize;
use serde_json::{json, Value};

const USAGE: &str = "usage: baseplate <op>\n  java-test analyze   {paths}                      \
     -> per-path is_java_test\n  patterns  match     {pattern, content, case_insensitive?} -> \
     compiles + matches\n  registry  load      {registry_yaml, overrides_yaml?}      -> parsed \
     summary\nreads a JSON request on stdin, writes an ADR-0052 response envelope on stdout.";

/// An `ok`-status ADR-0052 envelope wrapping a computed body.
fn ok_envelope(body: Value) -> String {
    json!({ "schema_version": "1", "status": "ok", "body": body }).to_string()
}

/// An `error`-status envelope — the ADR-0052 sentinel. A consumer treats `status != "ok"` as an
/// infrastructure failure and falls back to its in-process path rather than trusting a result.
fn error_envelope(message: &str) -> String {
    json!({ "schema_version": "1", "status": "error", "body": { "message": message } }).to_string()
}

/// A temp file that removes itself on drop, on every path including error and panic. The registry
/// loader takes a `&Path`, so inlined stdin content is materialized to a temp file rather than
/// requiring the caller to mount one.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Write `content` to a uniquely-named temp file.
///
/// Uses `create_new` (O_EXCL) rather than `fs::write`: the path is predictable, so on a shared
/// host (the brew/`cargo install` binary, not the isolated container) `fs::write` would *follow* a
/// pre-existing attacker symlink. O_EXCL makes an existing path a hard error instead — the op then
/// returns the fail-open error envelope, never a write through a symlink.
///
/// The name carries a monotonic sequence rather than a hand-written per-op nonce: O_EXCL turns a
/// name collision into a hard error, so two calls sharing a pid must not share a name.
fn write_temp(content: &str, label: &str) -> Result<TempFile, String> {
    use std::io::Write as _;
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "baseplate-{label}-{}-{seq}.yaml",
        std::process::id()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| format!("create temp {label} file: {e}"))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("write temp {label} file: {e}"))?;
    Ok(TempFile(path))
}

fn run_java_test_analyze(input: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Req {
        // No `#[serde(default)]`: an absent `paths` must not answer `results: []` — the same body
        // an explicitly empty list produces, i.e. "none of them" for a list that was never read.
        paths: Vec<String>,
    }
    let req: Req =
        serde_json::from_str(input).map_err(|e| format!("invalid java-test request JSON: {e}"))?;
    let results: Vec<Value> = req
        .paths
        .iter()
        .map(|p| json!({ "path": p, "is_java_test": java_test::is_java_test_file(p) }))
        .collect();
    Ok(ok_envelope(json!({ "results": results })))
}

fn run_patterns_match(input: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Req {
        pattern: String,
        // No `#[serde(default)]`, for the reason an absent `pattern` is already a hard error: an
        // absent `content` searched as `""` answers `match_count: 0` — "the pattern is not
        // present" — for a body the caller never supplied.
        content: String,
        #[serde(default)]
        case_insensitive: bool,
    }
    let req: Req =
        serde_json::from_str(input).map_err(|e| format!("invalid patterns request JSON: {e}"))?;
    // A pattern that fails to compile is a legitimate *result* (`compiles: false`), not an
    // infrastructure error — the caller asked "does this pattern work", and the answer is no.
    let compiled = if req.case_insensitive {
        patterns::compile_ci(&req.pattern)
    } else {
        patterns::compile_cs(&req.pattern)
    };
    let body = match compiled {
        Ok(re) => {
            let matches: Vec<Value> = re
                .find_iter(&req.content)
                .map(|m| json!({ "start": m.start(), "end": m.end(), "text": m.as_str() }))
                .collect();
            json!({ "compiles": true, "match_count": matches.len(), "matches": matches })
        }
        Err(e) => json!({ "compiles": false, "error": e.to_string(), "matches": [] }),
    };
    Ok(ok_envelope(body))
}

/// The promise IDs an overrides document names, or why it carries none.
///
/// Mirrors `registry::apply_overrides`'s reading of the document — a top-level `overrides:`
/// mapping, keyed by promise id — so that the two discards it makes silently become a stated
/// result instead. The merge itself stays the library's.
fn overridden_ids(text: &str) -> Result<Vec<String>, String> {
    let root: serde_yaml::Value = serde_yaml::from_str(text)
        .map_err(|e| format!("overrides document does not parse as YAML: {e}"))?;
    let overrides = root.get("overrides").and_then(|o| o.as_mapping()).ok_or(
        "overrides document has no `overrides:` mapping at its root, so nothing in it \
                would be applied (the overrides schema is not the registry schema)",
    )?;
    Ok(overrides
        .keys()
        .filter_map(|k| k.as_str().map(str::to_string))
        .collect())
}

fn run_registry_load(input: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Req {
        registry_yaml: String,
        #[serde(default)]
        overrides_yaml: Option<String>,
    }
    let req: Req =
        serde_json::from_str(input).map_err(|e| format!("invalid registry request JSON: {e}"))?;

    // `registry::apply_overrides` drops a document it cannot use — to stderr when the YAML does not
    // parse, in silence when the root is not an `overrides:` mapping — and `valid` then reports the
    // *base* document's verdict for a request the caller had amended. Decide it here, where the
    // envelope can say so, and keep the library's fail-open merge untouched.
    let override_ids = match req.overrides_yaml.as_deref().map(overridden_ids) {
        Some(Err(why)) => return Ok(ok_envelope(json!({ "valid": false, "error": why }))),
        Some(Ok(ids)) => Some(ids),
        None => None,
    };

    // Both temp files are owned locals: they drop (and delete) at function end on every branch.
    let reg_tmp = write_temp(&req.registry_yaml, "registry")?;
    let ov_tmp = match &req.overrides_yaml {
        Some(content) => Some(write_temp(content, "overrides")?),
        None => None,
    };
    let ov_path = ov_tmp.as_ref().map(|t| t.0.as_path());

    // An invalid registry is a *result* (`valid: false` + why), not an infrastructure error.
    let body = match registry::load(&reg_tmp.0, ov_path) {
        Ok(reg) => {
            let promises: Vec<Value> = reg
                .promises
                .iter()
                .map(|(id, spec)| {
                    json!({
                        "id": id,
                        "type": spec.promise_type,   // PromiseType: Serialize, lowercase
                        "enabled": spec.enabled,
                        "method": spec.method_raw,
                        "description": spec.description,
                    })
                })
                .collect();
            let mut body = json!({
                "valid": true,
                "version": reg.version,
                "promise_count": promises.len(),
                "promises": promises,
            });
            // An overrides entry naming a promise the registry does not have is `continue`d without
            // a word — and is almost always a typo. Reported only when a document was supplied, so
            // the field's absence means "no overrides in the request", never "none were dropped".
            if let Some(ids) = &override_ids {
                let unknown: Vec<&String> = ids
                    .iter()
                    .filter(|id| !reg.promises.contains_key(*id))
                    .collect();
                body["overrides_unknown"] = json!(unknown);
            }
            body
        }
        Err(e) => json!({ "valid": false, "error": e.to_string() }),
    };
    Ok(ok_envelope(body))
}

/// A one-shot op: JSON request text in, ADR-0052 envelope (or an error message) out.
type Op = fn(&str) -> Result<String, String>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let op = (
        args.get(1).map(String::as_str),
        args.get(2).map(String::as_str),
    );
    let handler: Option<Op> = match op {
        (Some("java-test"), Some("analyze")) => Some(run_java_test_analyze),
        (Some("patterns"), Some("match")) => Some(run_patterns_match),
        (Some("registry"), Some("load")) => Some(run_registry_load),
        _ => None,
    };
    match handler {
        Some(run) => {
            let mut input = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut input) {
                println!("{}", error_envelope(&format!("failed to read stdin: {e}")));
                std::process::exit(1);
            }
            match run(&input) {
                Ok(out) => println!("{out}"),
                Err(e) => {
                    // status=error envelope AND non-zero exit → the consumer's fallback fires
                    // (fail-open) instead of trusting an empty result.
                    println!("{}", error_envelope(&e));
                    std::process::exit(1);
                }
            }
        }
        // Conventional exit-0 help (e.g. the Homebrew formula's smoke test); an unknown/missing
        // op is a misuse → usage on stderr, exit 2.
        None => match args.get(1).map(String::as_str) {
            Some("--help") | Some("-h") | Some("help") => println!("{USAGE}"),
            _ => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_of(out: &str) -> Value {
        let v: Value = serde_json::from_str(out).expect("output is JSON");
        assert_eq!(v["schema_version"], "1");
        assert_eq!(v["status"], "ok");
        v["body"].clone()
    }

    #[test]
    fn java_test_classifies_test_and_main_paths() {
        let req = json!({"paths": ["src/test/java/x/FooTest.java", "src/main/java/x/Foo.java"]})
            .to_string();
        let body = body_of(&run_java_test_analyze(&req).unwrap());
        let results = body["results"].as_array().unwrap();
        assert_eq!(results[0]["is_java_test"], true);
        assert_eq!(results[1]["is_java_test"], false);
    }

    #[test]
    fn patterns_match_reports_hits_and_a_bad_pattern_is_a_result_not_an_error() {
        let hit = json!({"pattern": "TODO|FIXME", "content": "x TODO y"}).to_string();
        let body = body_of(&run_patterns_match(&hit).unwrap());
        assert_eq!(body["compiles"], true);
        assert!(body["match_count"].as_u64().unwrap() >= 1);

        // An uncompilable pattern is `compiles: false`, still an `ok` envelope — never a crash.
        let bad = json!({"pattern": "(unclosed", "content": "x"}).to_string();
        let body = body_of(&run_patterns_match(&bad).unwrap());
        assert_eq!(body["compiles"], false);
    }

    #[test]
    fn registry_load_summarizes_a_valid_registry_and_flags_an_invalid_one() {
        let yaml = "version: \"1\"\npromises:\n  read-before-write:\n    type: structural\n    \
                    method: grep\n    description: reads precede writes\n";
        let req = json!({"registry_yaml": yaml}).to_string();
        let body = body_of(&run_registry_load(&req).unwrap());
        assert_eq!(body["valid"], true);
        assert_eq!(body["version"], "1");
        assert!(body["promises"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "read-before-write" && p["type"] == "structural"));

        // A well-formed request whose registry lacks version/promises → `valid: false`, still ok.
        let invalid = json!({"registry_yaml": "foo: bar\n"}).to_string();
        let body = body_of(&run_registry_load(&invalid).unwrap());
        assert_eq!(body["valid"], false);
    }

    /// Deterministic, and deliberately not concurrent. The mutation that reverts `write_temp`
    /// to a fixed per-op nonce is only caught by the registry tests when libtest happens to run
    /// them in parallel — under `--test-threads=1`, or on a single-core runner, that mutant
    /// survives and a revert would ship silently. O_EXCL makes a shared name a hard error, so
    /// the property worth pinning is the name, not the race.
    #[test]
    fn two_temp_files_in_one_process_never_share_a_name() {
        let a = write_temp("x", "registry").unwrap();
        let b = write_temp("x", "registry").unwrap();
        assert_ne!(a.0, b.0, "two calls in one process must not share a name");
    }

    #[test]
    fn invalid_request_json_is_a_hard_error_not_a_false_clean_pass() {
        assert!(run_java_test_analyze("not json").is_err());
        assert!(run_patterns_match("not json").is_err());
        assert!(run_registry_load("not json").is_err());
    }

    /// The property both defects violate: for every field the USAGE marks required, omitting it
    /// must not answer the same thing an empty-but-supplied value answers. Absent `paths` used to
    /// read as "none of them", absent `content` as "not found" — a caller whose fetch returned
    /// nothing got a clean negative for a body that was never read.
    #[test]
    fn an_omitted_required_field_is_an_error_not_an_empty_result() {
        assert!(
            run_java_test_analyze("{}").is_err(),
            "absent paths must not answer results: []"
        );
        assert!(
            run_patterns_match(&json!({"pattern": "abc"}).to_string()).is_err(),
            "absent content must not answer match_count: 0"
        );

        // Controls: an explicitly empty value is a real input and still answers.
        let body = body_of(&run_java_test_analyze(&json!({"paths": []}).to_string()).unwrap());
        assert_eq!(body["results"].as_array().unwrap().len(), 0);
        let body = body_of(
            &run_patterns_match(&json!({"pattern": "abc", "content": ""}).to_string()).unwrap(),
        );
        assert_eq!(body["match_count"], 0);
    }

    /// The same property for `registry load`'s optional document: garbage in `overrides_yaml` used
    /// to be discarded inside the library (stderr at best, nothing at all for a wrong root key)
    /// while the envelope answered `valid: true` — the base document's verdict, for a request the
    /// caller had amended.
    #[test]
    fn a_discarded_overrides_document_is_reported_not_silently_dropped() {
        let yaml = "version: \"1\"\npromises:\n  a:\n    type: standing\n    method: grep\n    \
                    pattern: x\n    description: d\n";

        // Control: an overrides document that applies is `valid: true` and changes the answer.
        let good = json!({"registry_yaml": yaml, "overrides_yaml": "overrides:\n  a:\n    enabled: false\n"});
        let body = body_of(&run_registry_load(&good.to_string()).unwrap());
        assert_eq!(body["valid"], true);
        assert_eq!(body["promises"][0]["enabled"], false);
        assert_eq!(body["overrides_unknown"].as_array().unwrap().len(), 0);

        // Unparseable: not YAML at all.
        let bad = json!({"registry_yaml": yaml, "overrides_yaml": "[unclosed\n"});
        let body = body_of(&run_registry_load(&bad.to_string()).unwrap());
        assert_eq!(
            body["valid"], false,
            "an unparseable overrides document is not a valid load"
        );
        // Names *this* discard, not merely "something about overrides": a swallowed parse error
        // falls through to the no-root-mapping branch, which reports `valid: false` too.
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("does not parse as YAML"),
            "error was: {}",
            body["error"]
        );

        // Parses, but in the registry shape rather than the overrides shape — the silent branch.
        let wrong_shape = json!({"registry_yaml": yaml, "overrides_yaml": yaml});
        let body = body_of(&run_registry_load(&wrong_shape.to_string()).unwrap());
        assert_eq!(
            body["valid"], false,
            "an overrides document with no `overrides:` root is discarded"
        );
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("no `overrides:` mapping at its root"),
            "error was: {}",
            body["error"]
        );

        // Well-formed, but names a promise the registry does not have — almost always a typo.
        let typo = json!({"registry_yaml": yaml, "overrides_yaml": "overrides:\n  aa:\n    enabled: false\n"});
        let body = body_of(&run_registry_load(&typo.to_string()).unwrap());
        assert_eq!(body["valid"], true);
        assert_eq!(body["overrides_unknown"], json!(["aa"]));
    }
}
