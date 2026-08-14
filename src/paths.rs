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
/// `$BASEPLATE_HOME` is therefore **not** a blanket override: resolver 1 runs first, so a
/// binary that lives inside a git tree ignores it entirely. It is the escape hatch for the
/// installed-outside-a-tree case, which is the case resolver 1 cannot serve. Pinned by
/// `an_exe_inside_a_git_tree_ignores_baseplate_home`.
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
    fn an_exe_inside_a_git_tree_ignores_baseplate_home() {
        // The order IS the contract, and it was documented backwards (#20): CONTRACT.md said
        // "env override first" while `framework_root` tries `repo_root(current_exe())` first.
        // Nothing tested it — clean main with the two resolvers swapped was 36 passed, 0
        // failed — so both readings looked true. This is the test that is red on env-first.
        let _guard = PATHS_ENV_GUARD.lock().unwrap();

        let exe = std::env::current_exe().expect("a test binary has a path");
        let Some(exe_root) = repo_root(&exe) else {
            // Not a silent pass: on an unpacked .crate there is no `.git` above the binary,
            // so resolver 1 cannot win and there is no ordering to observe here.
            eprintln!(
                "SKIPPED an_exe_inside_a_git_tree_ignores_baseplate_home: no .git above {exe:?}"
            );
            return;
        };

        // An existing directory, so `baseplate_home_resolver` would accept it if reached.
        // Set only for the length of the call and restored immediately: under the correct
        // order nothing else can observe it, which is the property being asserted.
        let tmp = unique_tmp("ignored-home");
        let original = std::env::var("BASEPLATE_HOME").ok();
        std::env::set_var("BASEPLATE_HOME", &tmp.0);
        let precondition = baseplate_home_resolver();
        let resolved = framework_root();

        match original {
            Some(v) => std::env::set_var("BASEPLATE_HOME", v),
            None => std::env::remove_var("BASEPLATE_HOME"),
        }

        assert_eq!(
            precondition,
            Some(tmp.0.clone()),
            "precondition: this BASEPLATE_HOME must be resolvable, else the test proves nothing"
        );
        assert_eq!(
            resolved, exe_root,
            "resolver 1 (repo_root of the exe) must win over a perfectly good $BASEPLATE_HOME"
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
