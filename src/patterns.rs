use regex::{Regex, RegexBuilder};

/// Case-insensitive, unanchored regex. Use `find_iter` at the call site for
/// iterating all matches.
pub fn compile_ci(pat: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pat).case_insensitive(true).build()
}

/// Case-sensitive unanchored regex.
pub fn compile_cs(pat: &str) -> Result<Regex, regex::Error> {
    Regex::new(pat)
}

#[cfg(test)]
mod audit {
    use super::*;
    // Every pattern that ships in registry.yaml must compile under the regex crate.
    const REGISTRY_PATTERNS: &[&str] = &[
        "TODO|FIXME|HACK",
        "placeholder|not yet implemented|stub|dummy implementation",
        "try\\s*\\{|catch\\s*\\(|\\.catch\\(|\\.finally\\(",
        "\\.close\\(|\\.destroy\\(|\\.end\\(|finally\\s*\\{",
        "function |class |const |let |def |fn |pub |impl ",
        crate::java_test::JAVA_TEST_FILE_PATTERN, // §5.9 single source; see java_test::tests
        "\\.body\\(\\)\\.asString\\(\\)",
        "\\.body\\(\\)\\.print\\(\\)",
        "\\bJsonNode\\b",
        "\\bJsonPath\\b",
        "ObjectMapper\\(\\)\\.readTree",
        "\\bJSONObject\\b",
        "\\bJSONArray\\b",
        "\\bJSONTokener\\b",
        "\\.fields\\(\\)",
        "\\.getField\\(",
        "\\.rawJson\\(",
    ];
    #[test]
    fn all_registry_patterns_compile_case_sensitive() {
        for p in REGISTRY_PATTERNS {
            assert!(compile_cs(p).is_ok(), "pattern failed: {p}");
        }
    }
    #[test]
    fn grep_patterns_compile_case_insensitive() {
        for p in &REGISTRY_PATTERNS[0..5] {
            assert!(compile_ci(p).is_ok(), "ci pattern failed: {p}");
        }
    }
}
