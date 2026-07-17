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
/// "ok" / "retry-without-feedback" / "failed" / "dispatch-error".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewParser {
    Ok,
    RetryWithoutFeedback,
    Failed,
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
