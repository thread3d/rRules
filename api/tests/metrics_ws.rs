//! Integration tests for the real-time metrics WebSocket endpoint
//! `/v1/metrics/ws` (Plan 22-04, MON-02, P-4).
//!
//! Verifies the three behaviors required by the plan's `must_haves`:
//! - The WS handshake completes (HTTP 101 Switching Protocols).
//! - A broadcast on `metrics_tx` is delivered to the connected client as a
//!   serialized `MetricsSnapshot` WS text frame.
//! - The old JSON-stub response (`{"message":"WebSocket endpoint"...}`) is no
//!   longer returned at `/v1/metrics/ws` (RESEARCH § Pitfall 6).
//!
//! The test server mirrors `main.rs`'s `/v1` scope registration of
//! `configure_metrics_ws` and shares one `metrics_tx` app_data instance with
//! the test so the test can broadcast an `MetricsSnapshot` for the session to
//! forward. A raw TCP WS client performs the handshake and reads one frame so
//! the test does not depend on an extra WS client crate.

use std::time::Duration;

use actix_test::start;
use actix_web::{web, App};
use api::metrics::{create_metrics_channel, MetricsSnapshot, MetricsTx};
use api::websocket::configure_metrics_ws;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// RFC 6455 example Sec-WebSocket-Key (base64 of 16 bytes). The server echoes
/// back a Sec-WebSocket-Accept derived from this; the raw client does not
/// validate the accept, so any valid 16-byte key works.
const WS_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

/// Spin up a test server mirroring `main.rs`'s `/v1` scope registration of
/// `configure_metrics_ws`. Returns the bound address, the shared `metrics_tx`
/// (so the test can broadcast), a guard receiver kept alive for the test's
/// lifetime so `broadcast::send` never fails with "no receivers" before the WS
/// session subscribes (important for the RED state where no upgrade happens),
/// and the `TestServer` handle the caller must keep alive so the listener
/// stays open during the TCP client's connect.
fn spawn_server() -> (
    std::net::SocketAddr,
    MetricsTx,
    tokio::sync::broadcast::Receiver<MetricsSnapshot>,
    actix_test::TestServer,
) {
    let (metrics_tx, guard_rx) = create_metrics_channel(64);
    let metrics_tx_for_server = metrics_tx.clone();
    let server = start(move || {
        App::new()
            .app_data(web::Data::new(metrics_tx_for_server.clone()))
            .service(web::scope("/v1").configure(configure_metrics_ws))
    });
    let addr = server.addr().to_owned();
    (addr, metrics_tx, guard_rx, server)
}

/// Perform a raw WS handshake against `addr` at `/v1/metrics/ws`, broadcast one
/// `MetricsSnapshot` on `metrics_tx`, then read a single WS text frame and return
/// `(handshake_response, frame_payload_json)`. The broadcast is sent AFTER the
/// handshake so the session (when the upgrade succeeds) has a live receiver.
async fn handshake_then_broadcast_and_read_frame(
    addr: std::net::SocketAddr,
    metrics_tx: MetricsTx,
) -> (String, String) {
    let mut stream = TcpStream::connect(addr).await.unwrap();

    // Minimal WS opening handshake (RFC 6455 §4.1).
    let req = format!(
        "GET /v1/metrics/ws HTTP/1.1\r\n\
         Host: localhost\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         \r\n",
        key = WS_KEY
    );
    stream.write_all(req.as_bytes()).await.unwrap();

    // Read the handshake response up to the terminating \r\n\r\n. The server
    // either returns 101 (upgrade) or — in the RED state — a 200 JSON stub.
    let mut handshake_buf: Vec<u8> = Vec::new();
    loop {
        let mut one = [0u8; 1];
        if stream.read(&mut one).await.unwrap() == 0 {
            break;
        }
        handshake_buf.push(one[0]);
        if handshake_buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let handshake = String::from_utf8_lossy(&handshake_buf).to_string();

    // Broadcast an MetricsSnapshot the session should forward as a text frame.
    let state = MetricsSnapshot {
        engine_name: "test-engine".to_string(),
        rules_fired: 42,
        facts_inserted: 7,
        active_sessions: 1,
        memory_bytes: 1024,
        last_updated: "2026-07-02T00:00:00Z".to_string(),
    };
    // `send` returns Err only if there are zero receivers. The guard receiver
    // keeps it non-zero even in the RED state; in GREEN the session also holds
    // one. Either way this unwrap does not panic on "no receivers".
    let _ = metrics_tx.send(state);

    // Read a single WS frame from the server with a generous timeout. In
    // GREEN the session's 50ms poll forwards the broadcast within ~50ms; in
    // RED the server never upgrades and this times out (expected failure).
    let frame = read_one_ws_frame(&mut stream).await;
    (handshake, frame)
}

/// Read exactly one server→client WS frame and return its payload as a UTF-8
/// string. Server→client frames are NOT masked (RFC 6455 §5.3), so the mask
/// bit is 0. Handles both the 7-bit and 16-bit payload-length encodings.
async fn read_one_ws_frame(stream: &mut TcpStream) -> String {
    let mut buf: Vec<u8> = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        // If we have enough of the header, decode the total frame length.
        if buf.len() >= 2 {
            let len_marker = buf[1] & 0x7F;
            let (payload_len, header_len) = match len_marker {
                0..=125 => (len_marker as usize, 2),
                126 if buf.len() >= 4 => (u16::from_be_bytes([buf[2], buf[3]]) as usize, 4),
                126 => (0, 0), // header incomplete; need 4 bytes, keep reading
                _ => panic!("64-bit WS frame length not supported in this test"),
            };
            if header_len > 0 && buf.len() >= header_len + payload_len {
                let payload = buf[header_len..header_len + payload_len].to_vec();
                return String::from_utf8_lossy(&payload).to_string();
            }
        }
        if tokio::time::Instant::now() > deadline {
            panic!(
                "timed out waiting for a WS frame; got {} bytes: {:?}",
                buf.len(),
                String::from_utf8_lossy(&buf)
            );
        }
        let mut tmp = [0u8; 4096];
        match tokio::time::timeout(Duration::from_secs(1), stream.read(&mut tmp)).await {
            Ok(Ok(0)) => {
                if buf.is_empty() {
                    panic!("connection closed before any WS frame was received");
                }
                continue;
            }
            Ok(Ok(k)) => buf.extend_from_slice(&tmp[..k]),
            Ok(Err(e)) => panic!("tcp read error: {e}"),
            Err(_) => continue, // inner timeout; loop re-checks the deadline
        }
    }
}

#[actix_web::test]
async fn ws_handshake_completes_with_101() {
    let (addr, metrics_tx, _guard_rx, _server) = spawn_server();
    let (handshake, _frame) = handshake_then_broadcast_and_read_frame(addr, metrics_tx).await;
    assert!(
        handshake.starts_with("HTTP/1.1 101"),
        "expected HTTP/1.1 101 Switching Protocols, got: {handshake}"
    );
}

#[actix_web::test]
async fn ws_broadcast_delivers_engine_state_frame() {
    let (addr, metrics_tx, _guard_rx, _server) = spawn_server();
    let (_handshake, frame) = handshake_then_broadcast_and_read_frame(addr, metrics_tx).await;
    let parsed: serde_json::Value = serde_json::from_str(&frame).unwrap_or_else(|e| {
        panic!("frame is not valid JSON: {e}; frame={frame}")
    });
    assert_eq!(parsed["engine_name"], "test-engine", "frame={frame}");
    assert_eq!(parsed["rules_fired"], 42, "frame={frame}");
    assert_eq!(parsed["facts_inserted"], 7, "frame={frame}");
    assert_eq!(parsed["active_sessions"], 1, "frame={frame}");
    assert_eq!(parsed["memory_bytes"], 1024, "frame={frame}");
}

#[actix_web::test]
async fn ws_endpoint_is_no_longer_json_stub() {
    // The old stub returned `HTTP/1.1 200 OK` with a JSON body. A real WS
    // upgrade returns 101, so assert the status line is NOT 200 and the old
    // stub body is absent from the handshake response (Pitfall 6).
    let (addr, metrics_tx, _guard_rx, _server) = spawn_server();
    let (handshake, _frame) = handshake_then_broadcast_and_read_frame(addr, metrics_tx).await;
    assert!(
        !handshake.starts_with("HTTP/1.1 200"),
        "endpoint returned a plain 200 (the old JSON stub); handshake: {handshake}"
    );
    assert!(
        !handshake.contains("\"message\":\"WebSocket endpoint\""),
        "endpoint returned the old JSON stub body; handshake: {handshake}"
    );
}