//! `RmcpCxpakClient`'s live transport, against a fake `cxpak` on `PATH` (baseplate#8).
//!
//! The crate's existing tests cover `RecordedClient`, `is_indexing`, `SpawnBackoff` and DTO
//! defaults — all pure. The transport itself, which spawns a child process and owns three
//! timeouts and a kill path, had none. It is the code whose regressions surface in consumers
//! as a hang or a leaked child, which is exactly what is asserted here: **every case ends
//! with the child gone**, checked with `kill(pid, 0)` rather than inferred from the return
//! value.
//!
//! The stub is a POSIX shell script speaking just enough MCP over stdio to get rmcp through
//! `initialize`. That is what makes the connected cases reachable at all — without a
//! completed handshake there is no `ActiveConn`, and the eviction and Drop paths cannot be
//! entered.
//!
//! Cases select on the **work_dir**, which reaches the stub as argv `$3`
//! (`cxpak serve --mcp <work_dir>`). Not an env var: env is process-global and would race.
//!
//! Deliberately ONE test: it mutates `PATH` and `CXPAK_INDEX_WARM_BUDGET_MS`, both
//! process-global. Cargo gives each integration test file its own process. Do not add a
//! second test to this file.

use baseplate::cxpak::{CxpakClient, RmcpCxpakClient};
use serde_json::json;
use std::path::Path;
use std::time::{Duration, Instant};

/// Answers `initialize` for every case that needs a connection, then behaves per case.
/// Single-process on purpose (no `sleep`, no pipelines): the pid it records is the pid the
/// client kills, so "the child is gone" is a claim about the process we actually spawned.
const FAKE_CXPAK: &str = r#"#!/bin/sh
dir="$3"
echo $$ > "$dir.pid"
case "$dir" in
  *case-exit*) exit 1 ;;
esac
while IFS= read -r line; do
  case "$dir" in
    *case-mute*) continue ;;
  esac
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"method":"initialize"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"fake-cxpak","version":"0.0.0"}}}\n' "$id"
      ;;
    *'"method":"tools/call"'*)
      case "$dir" in
        *case-ok*)
          printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"{\\"answered\\":true}"}]}}\n' "$id"
          ;;
        # Answers cxpak's still-indexing sentinel on the FIRST call and real content on
        # the second, driving the retry loop through one poll and out the other side. The
        # counter lives in a file because each case gets a fresh process per connection.
        *case-index-then-ok*)
          n=$(cat "$dir.calls" 2>/dev/null || echo 0); echo $((n+1)) > "$dir.calls"
          if [ "$n" = 0 ]; then
            printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"cxpak: indexing in progress — Retry this call in a few seconds"}]}}\n' "$id"
          else
            printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"{\\"answered\\":true}"}]}}\n' "$id"
          fi
          ;;
        # Never warms. Drives the loop to budget expiry, which the test shortens via
        # CXPAK_INDEX_WARM_BUDGET_MS — the branch was otherwise a 25-second test.
        *case-index-forever*)
          n=$(cat "$dir.calls" 2>/dev/null || echo 0); echo $((n+1)) > "$dir.calls"
          printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"cxpak: indexing in progress — Retry this call in a few seconds"}]}}\n' "$id"
          ;;
        *) continue ;;
      esac
      ;;
    *) continue ;;
  esac
done
"#;

/// Running, as opposed to merely occupying a pid.
///
/// `kill(pid, 0)` alone is not enough here and would make this whole file vacuous in the
/// other direction: the client SIGKILLs the child but nothing awaits it, so the pid lingers
/// as a zombie (`Z <defunct>`) until the owning process exits, and signal 0 succeeds for a
/// zombie. Asserting on `kill(pid, 0)` reported the correctly-killed child as leaked.
#[cfg(unix)]
fn alive(pid: i32) -> bool {
    if unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    let out = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "state="])
        .output();
    match out {
        Ok(o) => {
            let state = String::from_utf8_lossy(&o.stdout);
            let state = state.trim();
            !state.is_empty() && !state.starts_with('Z')
        }
        // No ps: fall back to the weaker test rather than silently passing.
        Err(_) => true,
    }
}

/// How many `tools/call` requests the stub answered for `dir`.
fn calls(dir: &Path) -> u32 {
    std::fs::read_to_string(format!("{}.calls", dir.display()))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// The pid the stub recorded for `dir`, or None if it never started.
fn stub_pid(dir: &Path) -> Option<i32> {
    std::fs::read_to_string(format!("{}.pid", dir.display()))
        .ok()?
        .trim()
        .parse()
        .ok()
}

#[cfg(unix)]
fn assert_gone(dir: &Path, what: &str) {
    let Some(pid) = stub_pid(dir) else {
        panic!("{what}: the stub never recorded a pid — the case did not run");
    };
    for _ in 0..50 {
        if !alive(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    unsafe { libc::kill(pid, libc::SIGKILL) };
    panic!("{what}: cxpak child {pid} outlived the client");
}

#[tokio::test]
#[cfg(unix)]
async fn the_transport_never_leaves_a_cxpak_child_behind() {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!("baseplate-cxpak-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create stub dir");
    let stub = root.join("cxpak");
    std::fs::write(&stub, FAKE_CXPAK).expect("write stub");
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let prev_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", root.display(), prev_path));

    // 1. The child dies during the handshake. Error arm, not the timeout arm — it must not
    //    wait out HANDSHAKE_TIMEOUT to notice, so the elapsed time is part of the assertion.
    let exit_dir = root.join("case-exit");
    let t0 = Instant::now();
    let exited = RmcpCxpakClient::new(exit_dir.clone())
        .call("cxpak_context", json!({"op": "overview"}))
        .await;
    let exit_elapsed = t0.elapsed();

    // 2. A child that answers nothing: the 15s handshake timeout, then the kill.
    let mute_dir = root.join("case-mute");
    let muted = RmcpCxpakClient::new(mute_dir.clone())
        .call("cxpak_context", json!({"op": "overview"}))
        .await;

    // 3. Handshake completes, tool call never answered: the 10s call timeout, which evicts
    //    the connection and kills the child mid-life — the path a wedged cxpak takes.
    let wedged_dir = root.join("case-hang-after-init");
    let wedged = RmcpCxpakClient::new(wedged_dir.clone())
        .call("cxpak_context", json!({"op": "overview"}))
        .await;

    // 4. A connection that WORKS, then Drop. The only case where the child is alive and
    //    healthy at teardown, and the only one that exercises Drop's kill rather than an
    //    error path's. Scoped so the client drops before the assertion.
    let ok_dir = root.join("case-ok");
    let answered = {
        let client = RmcpCxpakClient::new(ok_dir.clone());
        let got = client
            .call("cxpak_context", json!({"op": "overview"}))
            .await;
        assert!(
            stub_pid(&ok_dir).map(alive).unwrap_or(false),
            "control: the child must still be running while the client holds it, or the \
             Drop assertion below proves nothing"
        );
        got
    };

    // 5. cxpak answers its still-indexing sentinel, then real content. The retry loop and
    //    its poll interval — the path a FIRST call after spawn takes on cxpak 3.1.x, which
    //    indexes in the background, and the one no case reached (baseplate#18).
    let warm_dir = root.join("case-index-then-ok");
    let warmed = RmcpCxpakClient::new(warm_dir.clone())
        .call("cxpak_context", json!({"op": "overview"}))
        .await;
    let warm_polls = calls(&warm_dir);

    // 6. Never warms: the budget expires and the call gives up. Shortened from 25s via the
    //    documented override, or this single case would dominate the suite's runtime.
    std::env::set_var("CXPAK_INDEX_WARM_BUDGET_MS", "1500");
    let cold_dir = root.join("case-index-forever");
    let t1 = Instant::now();
    let never_warm = RmcpCxpakClient::new(cold_dir.clone())
        .call("cxpak_context", json!({"op": "overview"}))
        .await;
    let cold_elapsed = t1.elapsed();
    let cold_polls = calls(&cold_dir);
    std::env::remove_var("CXPAK_INDEX_WARM_BUDGET_MS");

    std::env::set_var("PATH", prev_path);

    assert_eq!(
        warmed,
        Some(json!({"answered": true})),
        "a sentinel followed by content must be retried into a real answer, not returned \
         as junk — the sentinel is not JSON, so without the retry this is a parse failure"
    );
    assert_eq!(
        warm_polls, 2,
        "exactly one retry: a loop that did not poll would see 1 call, and one that \
         ignored the answer would keep going"
    );

    assert!(
        never_warm.is_none(),
        "a cxpak that never warms must give up, not hang"
    );
    assert!(
        cold_polls >= 2,
        "the budget case must have POLLED, not returned on the first sentinel — {cold_polls} call(s)"
    );
    assert!(
        cold_elapsed >= Duration::from_millis(1500) && cold_elapsed < Duration::from_secs(10),
        "must run out the shortened budget and stop there, not return early and not wait \
         out the 25s default (took {cold_elapsed:?})"
    );

    assert!(exited.is_none(), "a child that exits is not a completion");
    assert!(
        exit_elapsed < Duration::from_secs(10),
        "a dead child must fail the handshake immediately, not wait out the 15s timeout \
         (took {exit_elapsed:?})"
    );
    assert!(muted.is_none(), "a mute child must time out, not hang");
    assert!(wedged.is_none(), "an unanswered tool call must time out");
    assert!(
        answered.is_some(),
        "control: the happy path must return a value, or every 'is_none' above passes for \
         the wrong reason — a transport that never works trivially leaks nothing"
    );

    assert_gone(&exit_dir, "handshake error");
    assert_gone(&mute_dir, "handshake timeout");
    assert_gone(&wedged_dir, "call timeout eviction");
    assert_gone(&ok_dir, "Drop of a live connection");
    assert_gone(&warm_dir, "Drop after an index-warm retry");
    assert_gone(&cold_dir, "Drop after the index-warm budget expired");

    let _ = std::fs::remove_dir_all(&root);
}
