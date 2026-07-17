use std::path::{Path, PathBuf};

/// Walk up from `start` to the nearest ancestor containing a `.git` entry.
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

/// Resolves `$BASEPLATE_HOME`, if set, as the data root — but only when it names
/// an existing directory. This is the distribution-safe anchor for a binary
/// installed outside a git tree (`cargo install`, brew, `/usr/local/bin`):
/// unlike the cwd fallback, it can never silently resolve to whatever repo
/// happens to be the caller's working directory.
fn baseplate_home_resolver() -> Option<PathBuf> {
    let home = std::env::var("BASEPLATE_HOME").ok()?;
    let candidate = PathBuf::from(home);
    if candidate.is_dir() {
        Some(candidate)
    } else {
        None
    }
}

/// The data root a host application anchors its well-known directories under.
///
/// Resolution order (each only tried if the previous one fails):
/// 1. `repo_root(current_exe())` — stable across cwd; resolves to the repo a
///    binary that lives inside a git tree belongs to.
/// 2. `$BASEPLATE_HOME`, if it names an existing directory — the anchor for a
///    binary installed OUTSIDE any git tree (`cargo install` / brew), where
///    resolver 1 finds no `.git` above the installed binary.
/// 3. `repo_root(current_dir())`, else bare `current_dir()`, else `"."`.
///
/// The host application overrides resolution entirely by setting `$BASEPLATE_HOME`.
pub fn framework_root() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(root) = repo_root(&exe) {
            return root;
        }
    }
    if let Some(root) = baseplate_home_resolver() {
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

/// Absolute path to the global checkpoints registry.
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

    /// Serializes tests that mutate the process-global `BASEPLATE_HOME` env var.
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
            "baseplate-paths-test-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    #[test]
    fn repo_root_walks_up_to_git() {
        let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let Some(root) = repo_root(here) else {
            eprintln!("skip: no .git ancestor — unpacked/temp build");
            return;
        };
        assert!(root.join(".git").exists());
    }

    #[test]
    fn logs_dir_under_root() {
        assert!(logs_dir().ends_with("promise/logs"));
    }

    #[test]
    fn baseplate_home_resolves_when_dir_exists() {
        let _guard = PATHS_ENV_GUARD.lock().unwrap();
        let tmp = unique_tmp("home");

        let original = std::env::var("BASEPLATE_HOME").ok();
        std::env::set_var("BASEPLATE_HOME", &tmp.0);

        let resolved = baseplate_home_resolver();

        match original {
            Some(v) => std::env::set_var("BASEPLATE_HOME", v),
            None => std::env::remove_var("BASEPLATE_HOME"),
        }

        assert_eq!(
            resolved,
            Some(tmp.0.clone()),
            "BASEPLATE_HOME must resolve when it names an existing directory"
        );
    }

    #[test]
    fn baseplate_home_none_when_absent() {
        let _guard = PATHS_ENV_GUARD.lock().unwrap();

        let original = std::env::var("BASEPLATE_HOME").ok();
        std::env::set_var("BASEPLATE_HOME", "/nonexistent-baseplate-home-dir-xyz");

        let resolved = baseplate_home_resolver();

        match original {
            Some(v) => std::env::set_var("BASEPLATE_HOME", v),
            None => std::env::remove_var("BASEPLATE_HOME"),
        }

        assert_eq!(
            resolved, None,
            "a BASEPLATE_HOME that is not an existing directory must not resolve"
        );
    }
}
