//! Single source of truth for the Java test-file regex (spec §5.9).
//! Used by reviewer-skill routing and the registry
//! `test-assertion-deserialize.test_file_pattern` cross-check.

use once_cell::sync::Lazy;
use regex::Regex;

/// Maven/JUnit unit (`src/test/` + `*Test.java`/`*Tests.java`), Failsafe
/// integration (`*IT.java`), system-integration (`src/sit/` + `*SIT.java`),
/// and generic `tests/` (any depth). Canonical pattern; single source of truth.
pub const JAVA_TEST_FILE_PATTERN: &str =
    r"(?:^|/)(?:src/test/|src/sit/|tests?/).*\.java$|(?:Test|IT|Tests|SIT)\.java$";

static JAVA_TEST_FILE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(JAVA_TEST_FILE_PATTERN).expect("java-test regex is valid"));

/// True iff `path` looks like a Java test file. Normalizes `\` → `/` (Windows
/// paths); empty → false.
pub fn is_java_test_file(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let normalized = path.replace('\\', "/");
    JAVA_TEST_FILE_REGEX.is_match(&normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_maven_junit_sit_layouts() {
        for p in [
            "src/test/java/com/x/FooTest.java",
            "a/b/BarTests.java",
            "svc/FooIT.java",
            "src/sit/java/x/BazSIT.java",
            "tests/x/anything.java",
            "deep/tests/x/y/z.java",
        ] {
            assert!(is_java_test_file(p), "should match: {p}");
        }
        for p in [
            "src/main/java/com/x/Foo.java",
            "README.md",
            "src/x/Helper.java",
            "",
            "notatest.js",
        ] {
            assert!(!is_java_test_file(p), "should NOT match: {p}");
        }
    }
    #[test]
    fn normalizes_windows_backslashes() {
        assert!(is_java_test_file(r"src\test\java\com\x\FooTest.java"));
    }
}
