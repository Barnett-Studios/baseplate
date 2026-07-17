use crate::model::{Method, PromiseSpec, PromiseType, Requires};
use indexmap::IndexMap;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("read {0}: {1}")]
    Io(PathBuf, String),
    #[error("invalid registry at {0}: missing version or promises")]
    Invalid(PathBuf),
    #[error("parse {0}: {1}")]
    Parse(PathBuf, String),
    #[error("promise {0}: unknown standing method {1}")]
    UnknownStandingMethod(String, String),
}

/// Promise IDs for which the YAML entry had an explicit `requires:` key.
///
/// `available()` excludes any spec where the `requires` key is absent; a spec
/// with `requires: null` (key present, value null) IS included. Because serde
/// collapses both absent-key and `requires: null` to `Option::None`, key-presence
/// is tracked separately during load and used in `available()`.
pub struct Registry {
    pub version: String,
    pub promises: IndexMap<String, PromiseSpec>,
    /// IDs whose YAML spec contained the `requires:` key (even if the value is null).
    pub(crate) requires_keyed: std::collections::HashSet<String>,
}
pub struct Env {
    pub cxpak: bool,
    pub trace: bool,
}

fn method_from_str(s: &str) -> Option<Method> {
    serde_yaml::from_str::<Method>(s).ok()
}

pub fn load(
    registry_path: &Path,
    overrides_path: Option<&Path>,
) -> Result<Registry, RegistryError> {
    let text = std::fs::read_to_string(registry_path)
        .map_err(|e| RegistryError::Io(registry_path.into(), e.to_string()))?;
    let raw: serde_yaml::Value = serde_yaml::from_str(&text)
        .map_err(|e| RegistryError::Parse(registry_path.into(), e.to_string()))?;
    let version = raw
        .get("version")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let promises_node = raw.get("promises");
    let (version, promises_node) = match (version, promises_node) {
        (Some(v), Some(p)) if p.is_mapping() => (v, p),
        _ => return Err(RegistryError::Invalid(registry_path.into())),
    };

    let mut promises = IndexMap::new();
    let mut requires_keyed = std::collections::HashSet::new();
    for (k, spec_node) in promises_node.as_mapping().unwrap() {
        let id = k.as_str().unwrap_or_default().to_string();
        // Detect whether the `requires` key is present in this spec's YAML mapping.
        // `available()` skips any promise whose YAML entry has no `requires` key.
        if spec_node.get("requires").is_some() {
            requires_keyed.insert(id.clone());
        }
        let mut spec: PromiseSpec = serde_yaml::from_value(spec_node.clone())
            .map_err(|e| RegistryError::Parse(registry_path.into(), format!("{id}: {e}")))?;
        spec.id = id.clone();
        spec.enabled = true;
        if spec.promise_type == PromiseType::Standing {
            spec.method = Some(method_from_str(&spec.method_raw).ok_or_else(|| {
                RegistryError::UnknownStandingMethod(id.clone(), spec.method_raw.clone())
            })?);
        }
        promises.insert(id, spec);
    }

    if let Some(op) = overrides_path {
        apply_overrides(&mut promises, op);
    }
    Ok(Registry {
        version,
        promises,
        requires_keyed,
    })
}

pub fn apply_overrides(promises: &mut IndexMap<String, PromiseSpec>, path: &Path) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            eprintln!("[registry] failed to load {}: {e}", path.display());
            return;
        }
    };
    let root: serde_yaml::Value = match serde_yaml::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[registry] parse {}: {e}", path.display());
            return;
        }
    };
    let Some(overrides) = root.get("overrides").and_then(|o| o.as_mapping()) else {
        return;
    };
    for (k, patch) in overrides {
        let Some(id) = k.as_str() else { continue };
        let Some(spec) = promises.get_mut(id) else {
            continue;
        };
        if patch.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
            spec.enabled = false;
        }
        if let Some(t) = patch.get("threshold").and_then(|v| v.as_i64()) {
            spec.threshold = Some(t);
        }
    }
}

pub fn available<'a>(reg: &'a Registry, env: &Env) -> Vec<&'a PromiseSpec> {
    let mut out = Vec::new();
    for spec in reg.promises.values() {
        if !spec.enabled {
            continue;
        }
        // Promises whose YAML spec has no `requires` key are excluded.
        // Promises with `requires: null` (key present) ARE included.
        if !reg.requires_keyed.contains(&spec.id) {
            continue;
        }
        match &spec.requires {
            None => out.push(spec),
            Some(Requires::One(r)) if r == "cxpak" && env.cxpak => out.push(spec),
            Some(Requires::One(_)) => {}
            Some(Requires::List(rs)) => {
                let met = rs.iter().all(|r| match r.as_str() {
                    "cxpak" => env.cxpak,
                    "trace" => env.trace,
                    _ => false,
                });
                if met {
                    out.push(spec);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_version() {
        let dir = std::env::temp_dir().join("reg_test_novers");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("r.yaml");
        std::fs::write(&p, "promises: {}\n").unwrap();
        assert!(matches!(load(&p, None), Err(RegistryError::Invalid(_))));
        std::fs::remove_dir_all(&dir).ok();
    }
    #[test]
    fn rejects_unknown_standing_method() {
        let dir = std::env::temp_dir().join("reg_test_badmethod");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("r.yaml");
        std::fs::write(
            &p,
            "version: \"1\"\npromises:\n  x:\n    type: standing\n    method: bogus\n",
        )
        .unwrap();
        assert!(matches!(
            load(&p, None),
            Err(RegistryError::UnknownStandingMethod(_, _))
        ));
        std::fs::remove_dir_all(&dir).ok();
    }
    #[test]
    fn applies_overrides_disable_and_threshold() {
        let dir = std::env::temp_dir().join("reg_test_overrides");
        std::fs::create_dir_all(&dir).unwrap();
        let rp = dir.join("r.yaml");
        std::fs::write(&rp,
            "version: \"1\"\npromises:\n  a:\n    type: standing\n    method: grep\n    pattern: x\n    requires: null\n  b:\n    type: standing\n    method: output_length\n    requires: null\n").unwrap();
        let op = dir.join("overrides.yaml");
        std::fs::write(
            &op,
            "overrides:\n  a:\n    enabled: false\n  b:\n    threshold: 42\n",
        )
        .unwrap();
        let reg = load(&rp, Some(&op)).unwrap();
        assert!(!reg.promises["a"].enabled);
        assert_eq!(reg.promises["b"].threshold, Some(42));
        let active: Vec<_> = available(
            &reg,
            &Env {
                cxpak: false,
                trace: false,
            },
        )
        .iter()
        .map(|s| s.id.clone())
        .collect();
        assert!(!active.contains(&"a".to_string()) && active.contains(&"b".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }
    #[test]
    fn missing_overrides_file_is_silent() {
        // Self-contained (temp base registry): a missing *overrides* file must not
        // fail the load, independent of any particular file layout.
        let dir = std::env::temp_dir().join("reg_test_missing_ovr");
        std::fs::create_dir_all(&dir).unwrap();
        let rp = dir.join("r.yaml");
        std::fs::write(
            &rp,
            "version: \"1\"\npromises:\n  a:\n    type: standing\n    method: output_length\n    requires: null\n",
        )
        .unwrap();
        let bogus = std::path::Path::new("/nonexistent/overrides.yaml");
        assert!(load(&rp, Some(bogus)).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// FIX I-2: `available()` excludes promises whose YAML spec has NO `requires:` key.
    /// Promises with `requires: null` ARE included.
    /// This test uses a 3-promise registry to confirm the distinction is preserved.
    #[test]
    fn available_excludes_keyless_requires() {
        let dir = std::env::temp_dir().join("reg_test_keyless");
        std::fs::create_dir_all(&dir).unwrap();
        let rp = dir.join("r.yaml");
        // `with_null` has explicit `requires: null` → INCLUDED.
        // `keyless` has NO `requires` key at all → EXCLUDED (matches JS undefined behaviour).
        // `also_null` has explicit `requires: null` → INCLUDED.
        std::fs::write(
            &rp,
            concat!(
                "version: \"1\"\n",
                "promises:\n",
                "  with_null:\n    type: standing\n    method: output_length\n    requires: null\n",
                "  keyless:\n    type: standing\n    method: output_length\n",
                "  also_null:\n    type: standing\n    method: output_length\n    requires: null\n",
            ),
        )
        .unwrap();
        let reg = load(&rp, None).unwrap();
        let active: Vec<String> = available(
            &reg,
            &Env {
                cxpak: false,
                trace: false,
            },
        )
        .iter()
        .map(|s| s.id.clone())
        .collect();
        assert!(
            active.contains(&"with_null".to_string()),
            "requires: null must be included"
        );
        assert!(
            active.contains(&"also_null".to_string()),
            "requires: null must be included"
        );
        assert!(
            !active.contains(&"keyless".to_string()),
            "keyless requires must be excluded"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
