//! The happy path against a **real** `cxpak` (baseplate#8). `#[ignore]` — run with
//! `cargo test -- --ignored`, which is this assembly's convention for tests needing a live
//! binary on PATH.
//!
//! `cxpak_transport.rs` proves the transport's failure and teardown behaviour against a stub
//! that speaks the protocol as I understand it. That is precisely what it cannot prove: that
//! my understanding matches the real server. This does — one call, real handshake, real
//! index warm-up — and it is the test that fails when cxpak's wire format moves under us.
//!
//! It asserts the shape of an `overview` response only loosely, because the point is that a
//! value came back through the whole chain rather than what cxpak chose to say about this
//! repo today. A strict assertion here would be a fixture pinned to a tool that is not ours.

use baseplate::cxpak::{CxpakClient, RmcpCxpakClient};
use serde_json::json;

#[tokio::test]
#[ignore = "needs a real cxpak on PATH"]
async fn a_real_cxpak_answers_an_overview_through_the_live_transport() {
    let work_dir = std::env::current_dir().expect("cwd must be readable");
    let client = RmcpCxpakClient::new(work_dir);

    let got = client
        .call("cxpak_context", json!({"op": "overview"}))
        .await;

    let value = got.expect(
        "a real cxpak on PATH must answer an overview. None here means the handshake, the \
         index warm-up poll, or the response parse changed — the stub suite cannot see any \
         of those move, because it defines them",
    );
    assert!(
        value.is_object() || value.is_array(),
        "an overview must be structured context, got {value}"
    );
}
