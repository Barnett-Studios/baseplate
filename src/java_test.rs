//! Single source of truth for the Java test-file regex (spec §5.9).
//! Used by reviewer-skill routing and the registry
//! `test-assertion-deserialize.test_file_pattern` cross-check.

use once_cell::sync::Lazy;
use regex::Regex;

/// Maven/JUnit unit (`src/test/` + `*Test.java`/`*Tests.java`), Failsafe
/// integration (`*IT.java`), system-integration (`src/sit/` + `*SIT.java`),
/// and generic `tests/` (any depth). Canonical pattern; single source of truth.
///
/// ## Why `IT`/`SIT` carry a boundary and `Test`/`Tests` do not (baseplate#5)
///
/// The `IT` suffix used to be unanchored, so any name ENDING in those two letters matched:
/// `EXIT.java`, `UNIT.java`, `WAIT.java`, `ToolKIT.java`, `LoggerINIT.java`. That is a false
/// POSITIVE on a heuristic that routes reviewer selection and applies the test-code
/// carve-outs, so it hands ordinary classes the rules written for test code.
///
/// `[a-z0-9]` before the suffix is the boundary, because a Failsafe test is
/// `<PascalCaseName>IT` and the character before `IT` is therefore lower-case or a digit.
/// `ToolKIT` has an upper-case `K` there and is excluded; `FooIT` has an `o` and is not.
/// `^IT`/`/IT` keep a file named exactly `IT.java` matching.
///
/// `Test`/`Tests` are deliberately left unanchored, and that asymmetry is measured, not an
/// oversight: the capital `T` is already the boundary in PascalCase, so the collisions that
/// exist for `IT` do not exist for `Test` — `Latest.java`, `Contest.java`, `Manifest.java`
/// and `Protest.java` all end in a lower-case `test` and have never matched. Adding an
/// anchor there would change behaviour with no defect behind it, and would stop `Test.java`
/// matching.
///
/// Known and accepted: an ALL-CAPS test name (`FOOIT.java`, `TEST.java`) does not match.
/// That is a false NEGATIVE — it routes to the generic reviewer and applies no carve-outs —
/// which is the safe direction for this classifier, and Java does not name classes that way.
pub const JAVA_TEST_FILE_PATTERN: &str = r"(?:^|/)(?:src/test/|src/sit/|tests?/).*\.java$|(?:Test|Tests)\.java$|(?:[a-z0-9]|^|/)S?IT\.java$";

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

    /// The false positives baseplate#5 is about: an unanchored `IT` matched the TAIL of any
    /// word, so ordinary classes were handed the test-code carve-outs and routed to the test
    /// reviewer.
    ///
    /// `ToolKIT` and `LoggerINIT` are the cases the ticket's own examples miss — a PascalCase
    /// name ending in an acronym, which is how this actually appears in Java rather than as a
    /// SHOUTING filename.
    #[test]
    fn a_word_merely_ending_in_it_is_not_an_integration_test() {
        for p in [
            "EXIT.java",
            "AUDIT.java",
            "UNIT.java",
            "EDIT.java",
            "WAIT.java",
            "DEPOSIT.java",
            "TRANSIT.java",
            "ToolKIT.java",
            "LoggerINIT.java",
        ] {
            assert!(!is_java_test_file(p), "false positive: {p}");
        }
    }

    /// The ticket's acceptance criteria name these four, and ALL FOUR already classified
    /// correctly before the fix — the regex is case-sensitive and Java is PascalCase, so
    /// `Exit.java` ends in a lower-case `it` and never matched `IT`.
    ///
    /// Kept as a test rather than dropped as redundant: they are the acceptance criteria, and
    /// a criterion that was already satisfied is exactly the kind that gets cited as evidence
    /// a fix worked. They pass against the OLD regex too, and the test above is the one that
    /// does not.
    #[test]
    fn the_pascal_case_spellings_were_never_the_defect() {
        for p in ["Exit.java", "Audit.java", "Unit.java", "Deposit.java"] {
            assert!(!is_java_test_file(p), "false positive: {p}");
        }
    }

    /// …and the boundary must not cost a real test. `Foo2IT` covers the digit case, `IT.java`
    /// and `svc/IT.java` the start-of-name one.
    #[test]
    fn real_integration_tests_still_match_across_the_boundary_forms() {
        for p in [
            "FooIT.java",
            "Foo2IT.java",
            "BarSIT.java",
            "IT.java",
            "svc/IT.java",
            "src/sit/java/x/BazSIT.java",
        ] {
            assert!(is_java_test_file(p), "false negative: {p}");
        }
    }

    /// `Test`/`Tests` stay unanchored, and this is the measurement behind that asymmetry: the
    /// capital `T` is already the boundary in PascalCase, so the collision class that exists
    /// for `IT` does not exist here. Anchoring them would change behaviour with no defect
    /// behind it — and would stop `Test.java` matching.
    #[test]
    fn the_test_suffix_needs_no_boundary_because_capital_t_is_one() {
        for p in [
            "Latest.java",
            "Contest.java",
            "Manifest.java",
            "Protest.java",
        ] {
            assert!(!is_java_test_file(p), "false positive: {p}");
        }
        for p in [
            "BazTest.java",
            "GreatestTest.java",
            "QuxTests.java",
            "Test.java",
        ] {
            assert!(is_java_test_file(p), "false negative: {p}");
        }
    }
}
