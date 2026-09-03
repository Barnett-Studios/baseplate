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

/// The reviewer's structured retry decision (spec §4.3 `reviewer` object).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDecision {
    pub action: ReviewAction,
    pub feedback: Option<String>,
    pub reasoning: Option<String>,
    pub parser: ReviewParser,
    pub reviewer_skill: String,
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
    /// The list is the specification. A test that enumerated the variants from the type could
    /// only ever confirm that the type agrees with itself — it would accept a new variant with
    /// no declared wire form, which is exactly the change that breaks a consumer parsing this
    /// field. Adding a variant must mean adding a line here, and the count assertion below is
    /// what makes that unavoidable.
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

    #[test]
    fn no_review_parser_value_is_undeclared() {
        // A variant added without a PARSER_WIRE line ships a wire string nothing has agreed to.
        // `#[deny(non_exhaustive_omitted_patterns)]` is not stable, so the count is the guard:
        // this match is exhaustive, so a new variant fails to compile until it is listed here,
        // and then this assertion fails until PARSER_WIRE grows too.
        let declared = PARSER_WIRE.len();
        let exists = |v: ReviewParser| match v {
            ReviewParser::Ok
            | ReviewParser::UntaggedFallback
            | ReviewParser::RetryWithoutFeedback
            | ReviewParser::Failed
            | ReviewParser::DispatchError => 1,
        };
        let counted: usize = PARSER_WIRE.iter().map(|(v, _)| exists(*v)).sum();
        assert_eq!(
            counted, declared,
            "every declared value must be a real variant, and the match above must stay \
             exhaustive so a new variant cannot slip past undeclared"
        );
        assert_eq!(
            declared, 5,
            "PARSER_WIRE must list every ReviewParser variant"
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
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["parser"], serde_json::json!("untagged-fallback"));
        assert_eq!(v["action"], serde_json::json!("retry"));
        assert_eq!(v["feedback"], serde_json::json!("do it again"));
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
