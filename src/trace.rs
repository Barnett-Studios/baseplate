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

/// Build a trace event list from transcript messages.
///
/// Processes messages where `message.role == "assistant"` and `message.content`
/// is an array. For each `tool_use` block, emits a tool event with this
/// message's usage tokens. Wraps all events in a start/end pair and accumulates
/// total token counts.
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

        let usage = &msg["usage"];
        let in_tokens = usage["input_tokens"].as_u64().unwrap_or(0);
        let out_tokens = usage["output_tokens"].as_u64().unwrap_or(0);

        if in_tokens != 0 {
            total_in += in_tokens;
        }
        if out_tokens != 0 {
            total_out += out_tokens;
        }

        let content = match message["content"].as_array() {
            Some(a) => a,
            None => continue,
        };

        for block in content {
            if block["type"].as_str() != Some("tool_use") {
                continue;
            }

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
                    "in": in_tokens,
                    "out": out_tokens,
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
