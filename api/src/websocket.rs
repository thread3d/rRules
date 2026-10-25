//! WebSocket streaming for real-time monitoring (Plan 22-04, MON-02).
//!
//! Real WebSocket upgrade at `/v1/metrics/ws` broadcasting `MetricsSnapshot` from
//! the `metrics_tx` broadcast channel. Replaces the prior JSON-stub response
//! (RESEARCH § Pitfall 6) and is registered inside the `/v1` scope by
//! `main.rs::configure_metrics_ws`.
//!
//! The session actor subscribes to `metrics_tx` in `started` and polls the
//! broadcast channel on a short cadence, forwarding every buffered
//! `MetricsSnapshot` as a WS text frame (JSON-serialized). Data is legitimately
//! empty while `execute_single` is a stub (accepted tech debt per D-22-03);
//! the stream pushes the next `MetricsSnapshot` the engine broadcasts.

use std::time::Duration;

use actix::{Actor, AsyncContext, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws::{self, Message, ProtocolError, WebsocketContext};

use crate::metrics::MetricsTx;

/// Cadence at which the session polls the broadcast channel and forwards any
/// buffered `MetricsSnapshot` as a WS text frame. 50 ms keeps the dashboard
/// near-real-time without busy-looping; `try_recv` drains all buffered frames
/// each tick. A broadcast channel has no history, so frames sent before a
/// client connects are not replayed — the client receives the next state the
/// engine pushes.
const BROADCAST_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// WebSocket session actor streaming `MetricsSnapshot` frames to a connected
/// client.
///
/// Holds a clone of the broadcast `Sender` and subscribes in `started` so each
/// session gets its own `Receiver` (`broadcast::Receiver` is `!Clone`, so a
/// per-session `subscribe()` is the correct shape — the test broadcasts on the
/// shared `metrics_tx` and every connected session receives the frame).
struct MetricsWsSession {
    metrics_tx: MetricsTx,
}

impl Actor for MetricsWsSession {
    type Context = WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Subscribe to the broadcast channel. Each session gets its own
        // receiver so frames are not shared between connected clients.
        let mut rx = self.metrics_tx.subscribe();

        // Poll the broadcast channel on a short cadence and push every buffered
        // `MetricsSnapshot` as a WS text frame. When the channel is empty or lagged,
        // `try_recv` returns Err and the tick is a no-op.
        ctx.run_interval(
            BROADCAST_POLL_INTERVAL,
            move |_this: &mut Self, ctx: &mut WebsocketContext<Self>| {
                while let Ok(state) = rx.try_recv() {
                    let payload =
                        serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
                    ctx.text(payload);
                }
            },
        );
    }
}

/// Handle inbound WS frames. The metrics stream is read-only (the server
/// broadcasts `MetricsSnapshot`; no client input is accepted — see the threat
/// model), so everything except the protocol-level Ping/Close handshake is
/// ignored. Ping is answered with Pong (keep-alive); Close is echoed back so
/// the client sees a clean close.
impl StreamHandler<Result<Message, ProtocolError>> for MetricsWsSession {
    fn handle(&mut self, item: Result<Message, ProtocolError>, ctx: &mut Self::Context) {
        match item {
            Ok(Message::Ping(msg)) => ctx.pong(&msg),
            Ok(Message::Close(reason)) => ctx.close(reason),
            Ok(_) => (), // read-only broadcast: ignore Text/Binary/Pong/Continuation
            Err(_) => {} // protocol error: drop the frame, let the runtime handle it
        }
    }
}

/// WebSocket upgrade handler for `/v1/metrics/ws`.
///
/// Performs a real WebSocket upgrade via `actix_web_actors::ws::start`
/// (replaces the prior JSON-stub response — RESEARCH § Pitfall 6). The session
/// subscribes to `metrics_tx` and streams `MetricsSnapshot` frames as JSON text.
/// Registered inside the `/v1` scope by `configure_metrics_ws` so the full
/// path is `/v1/metrics/ws`.
pub async fn metrics_ws(
    req: HttpRequest,
    stream: web::Payload,
    metrics_tx: web::Data<MetricsTx>,
) -> Result<HttpResponse, actix_web::Error> {
    ws::start(
        MetricsWsSession {
            metrics_tx: metrics_tx.get_ref().clone(),
        },
        &req,
        stream,
    )
}

/// Configuration for the WebSocket monitoring endpoint. Registered inside the
/// `/v1` scope in `main.rs` so the flat route `/metrics/ws` resolves to
/// `/v1/metrics/ws`. Uses a flat route (matching `configure_metrics_dashboards`)
/// rather than a nested `web::scope("/metrics")` so the dashboard's existing
/// flat `/metrics/dead-rules` + `/metrics/coverage` routes and the WS route
/// share one `/v1/metrics/*` prefix without a scope/flat registration overlap.
pub fn configure_metrics_ws(cfg: &mut web::ServiceConfig) {
    cfg.route("/metrics/ws", web::get().to(metrics_ws));
}
