use std::path::{Path, PathBuf};

pub fn repo_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

/// Resolves `DOTCLAUDE_HOME`, if set, as the framework root — but only when it
/// actually contains a `promise/` directory. This is the distribution-safe
/// anchor for a binary installed outside a git tree (`cargo install`, brew,
/// `/usr/local/bin`): unlike the cwd fallback below, it can never silently
/// resolve to whatever repo happens to be the caller's working directory.
fn dotclaude_home_resolver() -> Option<PathBuf> {
    let home = std::env::var("DOTCLAUDE_HOME").ok()?;
    let candidate = PathBuf::from(home);
    if candidate.join("promise").is_dir() {
        Some(candidate)
    } else {
        None
    }
}

/// Resolves `$HOME/.claude` as the framework root when it exists and contains
/// a `promise/` directory — the framework's canonical home by convention, and
/// the second distribution-safe anchor (after `DOTCLAUDE_HOME`) tried before
/// ever falling back to a cwd-relative guess.
fn home_dot_claude_resolver() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let candidate = PathBuf::from(home).join(".claude");
    if candidate.join("promise").is_dir() {
        Some(candidate)
    } else {
        None
    }
}

/// Framework root = the repo containing this framework.
///
/// Resolution order (each only tried if the previous one fails):
/// 1. `repo_root(current_exe())` — stable across cwd; this is what resolves
///    correctly today for a binary living at `~/.claude/bin/dotclaude` inside
///    the `~/.claude` git tree.
/// 2. `DOTCLAUDE_HOME` env var, if it names a directory containing `promise/`.
/// 3. `$HOME/.claude`, if it contains `promise/` — the canonical home by
///    convention.
/// 4. `repo_root(current_dir())`, else bare `current_dir()`, else `"."`.
///
/// Resolvers 2-4 exist for a binary installed OUTSIDE any git tree (`cargo
/// install` / brew / `/usr/local/bin`): without them, resolver 1 fails (no
/// `.git` above the installed binary) and the OLD code fell straight through
/// to the cwd walk-up, which silently resolves to whatever git repo the
/// caller happens to be standing in — see `global_checkpoints_path()`'s
/// consumer in `dotclaude/src/main.rs` for why that is dangerous (the global
/// HITL checkpoint baseline would vanish without any diagnostic).
pub fn framework_root() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(root) = repo_root(&exe) {
            return root;
        }
    }
    if let Some(root) = dotclaude_home_resolver() {
        return root;
    }
    if let Some(root) = home_dot_claude_resolver() {
        return root;
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(root) = repo_root(&cwd) {
            return root;
        }
        return cwd;
    }
    PathBuf::from(".")
}

pub fn logs_dir() -> PathBuf {
    framework_root().join("promise/logs")
}

pub fn registry_path() -> PathBuf {
    framework_root().join("promise/registry.yaml")
}

pub fn overrides_path() -> PathBuf {
    framework_root().join("promise/overrides.yaml")
}

pub fn review_skill_path() -> PathBuf {
    framework_root().join("promise/skills/review-deterministic-findings.md")
}

pub fn test_code_reviewer_path() -> PathBuf {
    framework_root().join("agents/test-code-reviewer.md")
}

/// Absolute path to the global checkpoints registry shipped with the framework.
pub fn global_checkpoints_path() -> PathBuf {
    framework_root().join("promise/checkpoints.yaml")
}

/// Absolute path to the repo-local checkpoints registry under `repo_root`.
pub fn repo_checkpoints_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".dotclaude/checkpoints.yaml")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that mutate the process-global `DOTCLAUDE_HOME` /
    /// `HOME` env vars — mirrors the `INCONCLUSIVE_ENV_GUARD` pattern in
    /// `dotclaude-measure/src/run.rs`.
    static PATHS_ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A temp dir that removes itself on drop, including on panic.
    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn unique_tmp(label: &str) -> TempDir {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "dotclaude-paths-test-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    #[test]
    fn repo_root_walks_up_to_git() {
        // A git checkout (the standalone repo, or CI) is under a .git repo; skip
        // when built from an unpacked tarball or temp copy that has no .git.
        let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let Some(root) = repo_root(here) else {
            eprintln!("skip: no .git ancestor — unpacked/temp build");
            return;
        };
        assert!(root.join(".git").exists());
    }

    #[test]
    fn logs_dir_under_framework_root() {
        assert!(logs_dir().ends_with("promise/logs"));
    }

    // ── BUG4: DOTCLAUDE_HOME / $HOME/.claude resolvers ────────────────────────
    //
    // `framework_root()` itself cannot be exercised end-to-end for these cases:
    // its FIRST resolver (`repo_root(current_exe())`) always wins in this test
    // binary, because the binary is built inside the `~/.claude` git tree
    // (which happens to be the correct framework root here anyway), and
    // `current_exe()` cannot be faked in-process. Testing the private resolver
    // functions directly exercises exactly the logic `framework_root()` plugs
    // in as its 2nd/3rd resolvers, without that confound.

    #[test]
    fn dotclaude_home_env_resolves_when_promise_dir_present() {
        let _guard = PATHS_ENV_GUARD.lock().unwrap();
        let tmp = unique_tmp("dotclaude-home");
        std::fs::create_dir_all(tmp.0.join("promise")).expect("create promise dir");

        let original = std::env::var("DOTCLAUDE_HOME").ok();
        std::env::set_var("DOTCLAUDE_HOME", &tmp.0);

        let resolved = dotclaude_home_resolver();

        match original {
            Some(v) => std::env::set_var("DOTCLAUDE_HOME", v),
            None => std::env::remove_var("DOTCLAUDE_HOME"),
        }

        assert_eq!(
            resolved,
            Some(tmp.0.clone()),
            "DOTCLAUDE_HOME must be honoured when it contains a promise/ dir"
        );
    }

    #[test]
    fn dotclaude_home_env_ignored_when_promise_dir_absent() {
        let _guard = PATHS_ENV_GUARD.lock().unwrap();
        let tmp = unique_tmp("dotclaude-home-empty");
        // No promise/ subdir created — this is not a valid framework root.

        let original = std::env::var("DOTCLAUDE_HOME").ok();
        std::env::set_var("DOTCLAUDE_HOME", &tmp.0);

        let resolved = dotclaude_home_resolver();

        match original {
            Some(v) => std::env::set_var("DOTCLAUDE_HOME", v),
            None => std::env::remove_var("DOTCLAUDE_HOME"),
        }

        assert_eq!(
            resolved, None,
            "DOTCLAUDE_HOME without a promise/ dir must not be trusted as the framework root"
        );
    }

    #[test]
    fn home_dot_claude_resolver_finds_fake_home_with_promise_dir() {
        let _guard = PATHS_ENV_GUARD.lock().unwrap();
        let tmp = unique_tmp("fake-home");
        std::fs::create_dir_all(tmp.0.join(".claude/promise")).expect("create promise dir");

        let original = std::env::var("HOME").ok();
        std::env::set_var("HOME", &tmp.0);

        let resolved = home_dot_claude_resolver();

        match original {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        assert_eq!(
            resolved,
            Some(tmp.0.join(".claude")),
            "$HOME/.claude must resolve when it contains a promise/ dir"
        );
    }

    #[test]
    fn home_dot_claude_resolver_none_when_no_promise_dir() {
        let _guard = PATHS_ENV_GUARD.lock().unwrap();
        let tmp = unique_tmp("fake-home-empty");
        // No .claude/promise created.

        let original = std::env::var("HOME").ok();
        std::env::set_var("HOME", &tmp.0);

        let resolved = home_dot_claude_resolver();

        match original {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        assert_eq!(
            resolved, None,
            "a $HOME/.claude without a promise/ dir must not be trusted as the framework root"
        );
    }
}
