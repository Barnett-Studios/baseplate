use serde_json::{json, Value};

const MAX_ARGS_LENGTH: usize = 80;

fn is_leap(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

pub fn ts_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_ms = dur.as_millis() as u64;
    let total_s = total_ms / 1000;
    let frac_ms = total_ms % 1000;

    let time_of_day = (total_s % 86400) as u32;
    let h = time_of_day / 3600;
    let mi = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    let mut remaining_days = (total_s / 86400) as u32;
    let mut year = 1970u32;
    loop {
        let days_in_year = if is_leap(year) { 366u32 } else { 365u32 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let month_lengths: [u32; 12] = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for (i, &ml) in month_lengths.iter().enumerate() {
        if remaining_days < ml {
            month = i as u32 + 1;
            break;
        }
        remaining_days -= ml;
    }
    let day = remaining_days + 1;

    format!("{year:04}-{month:02}-{day:02}T{h:02}:{mi:02}:{s:02}.{frac_ms:03}Z")
}

/// Slice to `MAX_ARGS_LENGTH` UTF-16 code units, not bytes.
/// Byte slicing would panic on a non-char-boundary — aborting the hook and
/// breaking the exit-0 contract.
fn truncate(s: &str) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    if units.len() <= MAX_ARGS_LENGTH {
        s.to_string()
    } else {
        // ponytail: from_utf16_lossy yields U+FFFD if unit 77 splits a surrogate
        // pair; JS keeps the lone surrogate. Pathological (astral char exactly on
        // the boundary) and beyond corpus scope — the point of this fix is the
        // panic, not lone-surrogate fidelity.
        format!(
            "{}...",
            String::from_utf16_lossy(&units[..MAX_ARGS_LENGTH - 3])
        )
    }
}

/// Summarize a tool-call argument object for the trace.
///
/// Priority: `file_path` → `pattern` (joined with `path` if present) →
/// `command` → `query` → `url` → first key `"key:JSON(value)"`.
/// Empty or absent input returns `""`. Result is truncated to 80 UTF-16 units.
pub fn summarize_args(input: &Value) -> String {
    let obj = match input.as_object() {
        Some(o) if !o.is_empty() => o,
        _ => return String::new(),
    };

    if let Some(fp) = obj.get("file_path").and_then(|v| v.as_str()) {
        return truncate(fp);
    }
    if let Some(pattern) = obj.get("pattern").and_then(|v| v.as_str()) {
        let combined = match obj.get("path").and_then(|v| v.as_str()) {
            Some(path) => format!("{pattern} {path}"),
            None => pattern.to_string(),
        };
        return truncate(&combined);
    }
    if let Some(cmd) = obj.get("command").and_then(|v| v.as_str()) {
        return truncate(cmd);
    }
    if let Some(query) = obj.get("query").and_then(|v| v.as_str()) {
        return truncate(query);
    }
    if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
        return truncate(url);
    }

    if let Some((key, val)) = obj.iter().next() {
        let serialized = serde_json::to_string(val).unwrap_or_default();
        return truncate(&format!("{key}:{serialized}"));
    }

    String::new()
}

/// Input parameters for `build_fingerprint`.
pub struct FingerprintInput {
    pub model: String,
    pub claude_md_hash: String,
    pub rules_loaded: Vec<String>,
    pub promises_active: u64,
    pub registry_version: String,
    pub cxpak_available: bool,
    pub cxpak_version: Option<String>,
}

/// Build the fingerprint event that opens a trace file.
pub fn build_fingerprint(fp: FingerprintInput) -> Value {
    let cxpak_version: Value = if fp.cxpak_available {
        fp.cxpak_version.map(Value::String).unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    json!({
        "ts": ts_now(),
        "ev": "fingerprint",
        "model": fp.model,
        "claude_md_hash": fp.claude_md_hash,
        "rules_loaded": fp.rules_loaded,
        "promises_active": fp.promises_active,
        "registry_version": fp.registry_version,
        "cxpak_available": fp.cxpak_available,
        "cxpak_version": cxpak_version,
    })
}

/// Split a message's token count across the `tool_use` blocks it produced.
///
/// Attribution of a SHARED cost, not measurement of a per-block one: the model billed the
/// message once, and there is no per-block figure to recover. What this guarantees is the
/// property that makes the split usable — the parts sum to the whole, exactly. `total / n`
/// alone loses `total % n` tokens per message, so any consumer adding up tool events would
/// find less than the `end` event reports and have no way to tell rounding from a dropped
/// event. The remainder goes to the earliest blocks, one each.
///
/// `n == 0` never reaches here (the caller only splits when it has blocks), but it returns an
/// empty vec rather than dividing by zero, because a panic in a telemetry path takes the hook
/// with it.
fn split_tokens(total: u64, n: usize) -> Vec<u64> {
    if n == 0 {
        return Vec::new();
    }
    let n64 = n as u64;
    let base = total / n64;
    let extra = (total % n64) as usize;
    (0..n).map(|i| base + u64::from(i < extra)).collect()
}

/// Build a trace event list from transcript messages.
///
/// Processes messages where `message.role == "assistant"` and `message.content`
/// is an array. For each `tool_use` block, emits a tool event carrying that block's
/// SHARE of the message's usage. Wraps all events in a start/end pair and accumulates
/// total token counts.
///
/// ## Where `usage` lives (baseplate#6)
///
/// `message.usage`, at the same level as `role` and `content` — NOT `msg.usage` on the outer
/// wrapper, which is where this used to read it. Measured across 595,193 records from 125
/// Claude Code transcripts: `usage` appears under `message` 216,329 times and at the top
/// level **zero** times. So the old read was not "likely" wrong as filed, it was
/// unconditionally wrong, and `total_tokens` was always `{in: 0, out: 0}`.
///
/// There is deliberately NO fallback to the outer level. A fallback would let a fixture
/// written in a shape no producer emits keep passing, which is exactly how this survived:
/// the golden corpus asserting this function's output supplies `usage` at the top level,
/// so the fixtures and the code agreed with each other and neither agreed with Claude Code.
///
/// ## What `in` counts
///
/// `usage.input_tokens` only — uncached input. On the same corpus that is 0.09% of all input
/// tokens; `cache_read_input_tokens` is the other 99.9%. That is a narrower number than
/// "cost visibility" needs and it is NOT changed here: widening it would silently redefine a
/// published field. Filed separately.
pub fn build_trace(messages: &[Value]) -> Vec<Value> {
    let mut events: Vec<Value> = Vec::new();
    let mut total_in: u64 = 0;
    let mut total_out: u64 = 0;

    events.push(json!({ "ts": ts_now(), "ev": "start" }));

    for msg in messages {
        let message = &msg["message"];
        if message.is_null() || message["role"].as_str() != Some("assistant") {
            continue;
        }

        let usage = &message["usage"];
        let in_tokens = usage["input_tokens"].as_u64().unwrap_or(0);
        let out_tokens = usage["output_tokens"].as_u64().unwrap_or(0);

        // Accumulated ONCE per message, from the message's own usage — the totals are not
        // affected by how many blocks the split below produces.
        total_in += in_tokens;
        total_out += out_tokens;

        let content = match message["content"].as_array() {
            Some(a) => a,
            None => continue,
        };

        let uses: Vec<&Value> = content
            .iter()
            .filter(|b| b["type"].as_str() == Some("tool_use"))
            .collect();
        // The whole message's tokens used to be stamped on EVERY tool_use block, so a
        // message with k concurrent calls reported k× its real cost to anyone summing tool
        // events. Rare in practice — 1 message in 216,329 on the corpus above carried more
        // than one block, at most 3 — but the inflation is unbounded in k and free to fix.
        let ins = split_tokens(in_tokens, uses.len());
        let outs = split_tokens(out_tokens, uses.len());

        for (i, block) in uses.iter().enumerate() {
            let block_input = &block["input"];
            let args_summary = summarize_args(block_input);
            let file_path = block_input.get("file_path").cloned().unwrap_or(Value::Null);
            let name = block["name"].as_str().unwrap_or("").to_string();

            events.push(json!({
                "ts": ts_now(),
                "ev": "tool",
                "name": name,
                "args_summary": args_summary,
                "file_path": file_path,
                "tokens": {
                    "in": ins[i],
                    "out": outs[i],
                },
            }));
        }
    }

    events.push(json!({
        "ts": ts_now(),
        "ev": "end",
        "total_tokens": { "in": total_in, "out": total_out },
    }));

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The transcript shape Claude Code actually emits: `usage` beside `role` and
    /// `content`, under `message`.
    ///
    /// Hand-authored rather than copied from a real transcript — transcripts carry
    /// conversation content that has no business in a public repo — but the SHAPE is not
    /// invented: verified against 595,193 records from 125 Claude Code transcripts, where
    /// `usage` sits under `message` 216,329 times and at the top level zero times.
    fn assistant(tool_names: &[&str], input_tokens: u64, output_tokens: u64) -> Value {
        let content: Vec<Value> = tool_names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                serde_json::json!({
                    "type": "tool_use",
                    "id": format!("toolu_{i}"),
                    "name": n,
                    "input": {"file_path": "/w/f.rs"},
                })
            })
            .collect();
        serde_json::json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": content,
                "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens},
            }
        })
    }

    fn totals(events: &[Value]) -> (u64, u64) {
        let end = events.last().expect("an end event");
        assert_eq!(end["ev"], "end");
        (
            end["total_tokens"]["in"].as_u64().expect("in"),
            end["total_tokens"]["out"].as_u64().expect("out"),
        )
    }

    fn tool_tokens(events: &[Value]) -> Vec<(u64, u64)> {
        events
            .iter()
            .filter(|e| e["ev"] == "tool")
            .map(|e| {
                (
                    e["tokens"]["in"].as_u64().expect("in"),
                    e["tokens"]["out"].as_u64().expect("out"),
                )
            })
            .collect()
    }

    /// The filed defect: usage read from the outer wrapper is absent on every real record,
    /// so the totals were always zero and nothing said so.
    #[test]
    fn totals_are_nonzero_for_the_transcript_shape_claude_code_emits() {
        let events = build_trace(&[
            assistant(&["Read"], 150, 75),
            assistant(&["Edit"], 200, 100),
        ]);
        assert_eq!(totals(&events), (350, 175));
    }

    /// …and the shape the old code read is NOT silently accepted. A fallback would let a
    /// fixture written against the implementation keep passing, which is how the defect
    /// survived a golden corpus in the first place.
    #[test]
    fn usage_on_the_outer_wrapper_is_not_read() {
        let msg = serde_json::json!({
            "type": "assistant",
            "message": {"role": "assistant", "content": [
                {"type": "tool_use", "id": "t", "name": "Read", "input": {}}
            ]},
            "usage": {"input_tokens": 999, "output_tokens": 999},
        });
        let events = build_trace(std::slice::from_ref(&msg));
        assert_eq!(
            totals(&events),
            (0, 0),
            "usage at the top level is a shape no producer emits; reading it would keep the \
             defect alive behind a fallback"
        );
    }

    /// Concurrent tool calls: each block gets its SHARE, not the whole message.
    #[test]
    fn concurrent_tool_calls_split_the_message_tokens_rather_than_repeating_them() {
        let events = build_trace(&[assistant(&["Read", "Bash", "Grep"], 150, 75)]);
        let per = tool_tokens(&events);
        assert_eq!(per.len(), 3);
        assert_eq!(per, vec![(50, 25), (50, 25), (50, 25)]);
        // The old behaviour was (150,75) three times — 3× the real cost to anyone summing.
        assert_eq!(totals(&events), (150, 75));
    }

    /// The parts sum to the whole even when the split does not divide evenly, or a consumer
    /// adding up tool events silently under-counts and cannot tell rounding from a lost event.
    #[test]
    fn an_uneven_split_loses_no_tokens() {
        for (tokens, blocks) in [(100u64, 3usize), (7, 4), (1, 5), (0, 2), (999_983, 7)] {
            let names = vec!["Read"; blocks];
            let events = build_trace(&[assistant(&names, tokens, tokens)]);
            let per = tool_tokens(&events);
            assert_eq!(per.len(), blocks);
            let sum_in: u64 = per.iter().map(|(i, _)| i).sum();
            let sum_out: u64 = per.iter().map(|(_, o)| o).sum();
            assert_eq!(
                sum_in, tokens,
                "{tokens} over {blocks} blocks lost input tokens"
            );
            assert_eq!(
                sum_out, tokens,
                "{tokens} over {blocks} blocks lost output tokens"
            );
            // …and no block is starved by more than one token relative to any other.
            let max = per.iter().map(|(i, _)| *i).max().expect("blocks");
            let min = per.iter().map(|(i, _)| *i).min().expect("blocks");
            assert!(max - min <= 1, "the remainder must spread, not pile up");
        }
    }

    /// A message with usage and no tool_use blocks still counts toward the totals — the
    /// totals are the message's, and the split is only about attribution.
    #[test]
    fn a_message_with_no_tool_calls_still_counts_toward_the_totals() {
        let msg = serde_json::json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": "thinking out loud"}],
                "usage": {"input_tokens": 40, "output_tokens": 12},
            }
        });
        let events = build_trace(std::slice::from_ref(&msg));
        assert_eq!(totals(&events), (40, 12));
        assert!(tool_tokens(&events).is_empty());
    }

    #[test]
    fn truncate_multibyte_does_not_panic_and_uses_utf16_length() {
        // Regression: byte-slicing at unit 77 panicked when it split a multibyte
        // char, aborting the hook (exit-0 contract break). "é" is 2 bytes / 1
        // UTF-16 unit — 90 of them is 180 bytes but 90 UTF-16 units (> 80), so it
        // must truncate to 77 units + "..." without panicking.
        let long = "é".repeat(90);
        let v = serde_json::json!({ "file_path": &long });
        let r = summarize_args(&v);
        assert!(r.ends_with("..."), "must truncate: {r}");
        assert_eq!(
            r.encode_utf16().count(),
            80,
            "truncated result is 77 units + 3 for the ellipsis = 80 UTF-16 units"
        );

        // A 79-unit multibyte string is under the limit → returned verbatim.
        let short = "é".repeat(79);
        let v2 = serde_json::json!({ "file_path": &short });
        let r2 = summarize_args(&v2);
        assert_eq!(
            r2, short,
            "under-limit multibyte must pass through untouched"
        );
    }

    #[test]
    fn summarize_args_truncation_boundary() {
        // 79 chars: no truncation (79 <= 80)
        let s79 = "a".repeat(79);
        let v79 = serde_json::json!({"file_path": &s79});
        let r79 = summarize_args(&v79);
        assert_eq!(r79.len(), 79, "79-char path should not be truncated");
        assert!(!r79.ends_with("..."));

        // 80 chars: no truncation (80 <= 80)
        let s80 = "a".repeat(80);
        let v80 = serde_json::json!({"file_path": &s80});
        let r80 = summarize_args(&v80);
        assert_eq!(r80.len(), 80, "80-char path should not be truncated");
        assert!(!r80.ends_with("..."));

        // 81 chars: truncated to 77 + "..." = 80
        let s81 = "a".repeat(81);
        let v81 = serde_json::json!({"file_path": &s81});
        let r81 = summarize_args(&v81);
        assert_eq!(r81.len(), 80, "81-char path should be truncated to 80");
        assert!(r81.ends_with("..."), "truncated result must end with '...'");
        assert_eq!(&r81[..77], &s81[..77], "first 77 chars must be preserved");
    }
}
