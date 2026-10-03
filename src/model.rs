use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Observation {
    Kept,
    Broken,
    Partial,
    Skipped,
}

impl Observation {
    /// EMA observation value; None == excluded (skipped).
    pub fn value(&self) -> Option<f64> {
        match self {
            Observation::Kept => Some(1.0),
            Observation::Broken => Some(0.0),
            Observation::Partial => Some(0.5),
            Observation::Skipped => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn weight(&self) -> f64 {
        match self {
            Confidence::High => 1.0,
            Confidence::Medium => 0.6,
            Confidence::Low => 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromiseType {
    Standing,
    Structural,
    Behavioral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Grep,
    GrepAbsent,
    ConsecutiveComments,
    OutputLength,
    FileCheck,
    OutputContains,
    OutputStructure,
    TokenMetric,
    Timing,
    TestAssertionPatterns,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodOutcome {
    pub result: Observation,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub promise_id: String,
    pub method: String,
    pub confidence: Confidence,
    pub result: Observation,
    pub evidence: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Requires {
    One(String),
    List(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct PromiseSpec {
    #[serde(skip)]
    pub id: String,
    #[serde(rename = "type")]
    pub promise_type: PromiseType,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "method")]
    pub method_raw: String,
    #[serde(default)]
    pub confidence: Option<Confidence>,
    #[serde(default)]
    pub requires: Option<Requires>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub min_lines: Option<i64>,
    #[serde(default)]
    pub min_chars: Option<i64>,
    #[serde(default)]
    pub check: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<i64>,
    #[serde(default)]
    pub max_ms: Option<i64>,
    #[serde(default)]
    pub test_file_pattern: Option<String>,
    #[serde(default)]
    pub forbidden_patterns: Option<Vec<String>>,
    #[serde(default)]
    pub threshold: Option<i64>,
    #[serde(default)]
    pub tool_pattern: Option<String>,
    #[serde(skip)]
    pub method: Option<Method>,
}

fn default_enabled() -> bool {
    true
}

/// Reviewer decision action. Serializes lowercase: "accept" / "retry".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewAction {
    Accept,
    Retry,
}

/// Which code path produced the decision. Serializes kebab-case:
/// "ok" / "untagged-fallback" / "retry-without-feedback" / "failed" / "dispatch-error".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewParser {
    /// A decision was parsed from the tier the dispatch protected — for a tagged dispatch,
    /// from a candidate carrying the requested tag.
    Ok,
    /// A decision was parsed, but **a tag was requested and no tagged candidate was found**, so
    /// it came from the untagged fallback tier.
    ///
    /// This exists because `Ok` used to cover both, and the two are not the same claim
    /// (attestr#31). Tagging a review dispatch is what stops a verdict planted in the
    /// agent-authored text from outranking the reviewer's own — but the tag is emitted by a
    /// language model following a formatting instruction, and when it does not comply the parse
    /// falls back past the protected tier. Measured on published `attestr 0.4.1`: an untagged
    /// reviewer reply that quotes its input **after** its own verdict returns the planted
    /// `{"action":"accept"}` with `parser: Ok` — indistinguishable from a verdict the tag
    /// actually protected. From the parser's position a non-compliant reviewer and a consumer
    /// who never asked for a tag are the same input; this variant is what tells them apart.
    ///
    /// **The parse succeeded.** This is not a failure and must not be treated as one — the
    /// fail-open posture is deliberate and a missing tag does not become a hard error. It is
    /// lower-confidence telemetry: the decision is usable, and a consumer that cares about the
    /// mitigation having applied can now see that it did not.
    UntaggedFallback,
    /// The reviewer asked to retry but supplied no feedback to retry with.
    RetryWithoutFeedback,
    /// No decision could be parsed from the reviewer's reply.
    Failed,
    /// The reviewer was never reached.
    DispatchError,
}

/// Whether the reviewer ran on a harness distinct from the turn's author, as *observed* by
/// whoever constructed this decision (attestr#1).
///
/// This crate never decides the value of this field — per ADR-0002, attestr's verification
/// layer is telemetry, not control, and only the glue consumer knows both the author's and
/// the reviewer's actual identities and owns the cascade between them. This type exists so
/// that consumer has somewhere honest to record what it saw, rather than a free-text
/// substring of `ReviewDecision::reasoning` (the alternative attestr#1 rejected: a consumer
/// branching on prose breaks the moment the prose is reworded).
///
/// **A consumer must treat `Unknown` as "independence was not shown", never as evidence
/// either way, and must never gate any decision on `== SameHarness` alone** — `Unknown` is
/// the fail-open default (no comparison made, an identity missing, or data from before this
/// field existed), so a gate written the other way around (proceed unless `SameHarness`)
/// treats "nobody checked" as if it had been checked and passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Independence {
    /// The reviewer's harness differed from the author's.
    Independent,
    /// The reviewer's harness was the same as the author's — the condition attestr#1 exists
    /// to make visible, not the default assumption.
    SameHarness,
    /// No comparison was made, or either identity was unknown. Also what a record persisted
    /// before this field existed deserializes to (`#[serde(default)]` on
    /// `ReviewDecision::independence`) — "unknown" and "never observed" are deliberately the
    /// same value rather than two, so a reader cannot tell them apart and must not try to.
    #[default]
    Unknown,
}

/// The reviewer's structured retry decision (spec §4.3 `reviewer` object).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDecision {
    pub action: ReviewAction,
    pub feedback: Option<String>,
    pub reasoning: Option<String>,
    pub parser: ReviewParser,
    pub reviewer_skill: String,
    /// `#[serde(default)]`, not a required field: a record written before this field existed
    /// has no opinion on independence, and `Independence::Unknown` is exactly that — the
    /// same value a caller that never checked would report deliberately, not a sentinel for
    /// "this is old data".
    #[serde(default)]
    pub independence: Independence,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observation_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&Observation::Kept).unwrap(),
            "\"kept\""
        );
        assert_eq!(
            serde_json::to_string(&Observation::Skipped).unwrap(),
            "\"skipped\""
        );
    }
    #[test]
    fn observation_values_match_ema_contract() {
        assert_eq!(Observation::Kept.value(), Some(1.0));
        assert_eq!(Observation::Broken.value(), Some(0.0));
        assert_eq!(Observation::Partial.value(), Some(0.5));
        assert_eq!(Observation::Skipped.value(), None);
    }
    #[test]
    fn confidence_weights_match() {
        assert_eq!(Confidence::High.weight(), 1.0);
        assert_eq!(Confidence::Medium.weight(), 0.6);
        assert_eq!(Confidence::Low.weight(), 0.3);
    }
    #[test]
    fn method_parses_registry_strings() {
        let m: Method = serde_json::from_str("\"grep_absent\"").unwrap();
        assert_eq!(m, Method::GrepAbsent);
    }
    #[test]
    fn method_outcome_json_keys() {
        let o = MethodOutcome {
            result: Observation::Broken,
            evidence: "x".into(),
        };
        let v: serde_json::Value = serde_json::to_value(&o).unwrap();
        assert!(v.get("result").is_some() && v.get("evidence").is_some());
    }
    #[test]
    fn review_action_serializes_lowercase() {
        assert_eq!(
            serde_json::to_value(ReviewAction::Accept).unwrap(),
            serde_json::json!("accept")
        );
        assert_eq!(
            serde_json::to_value(ReviewAction::Retry).unwrap(),
            serde_json::json!("retry")
        );
    }
    /// Every `ReviewParser` value, with the wire string it must carry, written out here rather
    /// than derived from the enum.
    ///
    /// The list is the specification. Deriving it from the type could only ever confirm that
    /// the type agrees with itself; hand-writing it means a wire string has to be *declared*,
    /// and `every_review_parser_value_round_trips_its_wire_form` then holds serde to it.
    ///
    /// Completeness is the opposite problem and needs the opposite property — see
    /// `no_review_parser_value_is_undeclared`, which gets its denominator from serde rather
    /// than from this array.
    const PARSER_WIRE: [(ReviewParser, &str); 5] = [
        (ReviewParser::Ok, "ok"),
        (ReviewParser::UntaggedFallback, "untagged-fallback"),
        (ReviewParser::RetryWithoutFeedback, "retry-without-feedback"),
        (ReviewParser::Failed, "failed"),
        (ReviewParser::DispatchError, "dispatch-error"),
    ];

    #[test]
    fn every_review_parser_value_round_trips_its_wire_form() {
        for (value, wire) in PARSER_WIRE {
            assert_eq!(
                serde_json::to_value(value).unwrap(),
                serde_json::json!(wire),
                "{value:?} must serialize as {wire:?}"
            );
            let back: ReviewParser = serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(back, value, "{wire:?} must deserialize back to {value:?}");
        }
    }

    /// Every variant of the type must have a declared wire form.
    ///
    /// The denominator cannot come from `PARSER_WIRE` — an assertion that walks the declared
    /// list is blind to what is missing from it, by construction. The first version of this
    /// test summed an exhaustive match over `PARSER_WIRE`'s entries and claimed the match kept
    /// it honest; it did not. Adding a sixth variant and giving it an arm in that match left
    /// the suite green with an undeclared value in the type, which is the exact change this
    /// test exists to stop.
    ///
    /// `#[deny(non_exhaustive_omitted_patterns)]` and `std::mem::variant_count` are both
    /// unstable, and nothing on stable Rust forces a hand-written list to grow. But serde's
    /// own `unknown variant` error is generated by the derive macro **from the type**, so it
    /// is an independent enumeration and it is already here. That is the denominator.
    #[test]
    fn no_review_parser_value_is_undeclared() {
        let err = serde_json::from_str::<ReviewParser>("\"definitely-not-a-variant\"")
            .expect_err("an unknown wire string must not deserialize")
            .to_string();
        let listed = err
            .split_once("expected one of ")
            .map(|(_, rest)| rest)
            .unwrap_or_default();
        let from_serde: std::collections::BTreeSet<&str> =
            listed.split('`').skip(1).step_by(2).collect();

        // Positive control. This test reads a message serde does not promise the shape of;
        // if that shape changes, `from_serde` goes empty and an empty set would otherwise
        // still have to differ from `declared` — but it would differ for the wrong reason,
        // and the failure would read as a missing variant. Fail on the real cause instead.
        assert!(
            from_serde.contains("ok"),
            "serde's unknown-variant error no longer lists variants in the form this test \
             parses, so it is not enumerating anything: {err:?}"
        );

        let declared: std::collections::BTreeSet<&str> =
            PARSER_WIRE.iter().map(|(_, wire)| *wire).collect();
        assert_eq!(
            from_serde, declared,
            "every variant of the type must have a PARSER_WIRE line, and every line must name \
             a real variant"
        );
    }

    #[test]
    fn an_untagged_fallback_is_not_a_failure() {
        // The posture, asserted rather than left to the doc comment: this variant means the
        // parse SUCCEEDED through the unprotected tier. A consumer treating it as a parse
        // failure would turn attestr's fail-open review into a hard one (attestr#31).
        let d = ReviewDecision {
            action: ReviewAction::Retry,
            feedback: Some("do it again".into()),
            reasoning: None,
            parser: ReviewParser::UntaggedFallback,
            reviewer_skill: "s".into(),
            independence: Independence::Unknown,
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["parser"], serde_json::json!("untagged-fallback"));
        assert_eq!(v["action"], serde_json::json!("retry"));
        assert_eq!(v["feedback"], serde_json::json!("do it again"));
    }

    /// A `ReviewDecision` persisted before `independence` existed has no opinion on it, and
    /// `#[serde(default)]` must make that mean `Unknown` rather than a deserialize error —
    /// the whole point of adding a field to data someone already has on disk (attestr#1).
    #[test]
    fn a_pre_independence_record_deserializes_to_unknown() {
        let old = serde_json::json!({
            "action": "accept",
            "feedback": null,
            "reasoning": "looks fine",
            "parser": "ok",
            "reviewer_skill": "generic"
        });
        let d: ReviewDecision = serde_json::from_value(old)
            .expect("a record with no independence key must still deserialize");
        assert_eq!(d.independence, Independence::Unknown);
    }

    #[test]
    fn every_independence_value_round_trips_its_wire_form() {
        const WIRE: [(Independence, &str); 3] = [
            (Independence::Independent, "independent"),
            (Independence::SameHarness, "same-harness"),
            (Independence::Unknown, "unknown"),
        ];
        for (value, wire) in WIRE {
            assert_eq!(
                serde_json::to_value(value).unwrap(),
                serde_json::json!(wire),
                "{value:?} must serialize as {wire:?}"
            );
            let back: Independence = serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(back, value, "{wire:?} must deserialize back to {value:?}");
        }
    }

    #[test]
    fn independence_default_is_unknown() {
        assert_eq!(Independence::default(), Independence::Unknown);
    }

    #[test]
    fn review_parser_serializes_kebab_case() {
        assert_eq!(
            serde_json::to_value(ReviewParser::Ok).unwrap(),
            serde_json::json!("ok")
        );
        assert_eq!(
            serde_json::to_value(ReviewParser::RetryWithoutFeedback).unwrap(),
            serde_json::json!("retry-without-feedback")
        );
        assert_eq!(
            serde_json::to_value(ReviewParser::Failed).unwrap(),
            serde_json::json!("failed")
        );
        assert_eq!(
            serde_json::to_value(ReviewParser::DispatchError).unwrap(),
            serde_json::json!("dispatch-error")
        );
    }
}
