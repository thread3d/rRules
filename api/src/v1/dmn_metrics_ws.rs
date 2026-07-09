//! WebSocket broadcast for DMN metrics snapshots (Phase 33).
//!
//! Pattern: mirrors `api::websocket` (the engine-metrics WS) but uses the
//! dedicated `DmnMetricsTx` broadcast channel. Each connected client gets its
//! own receiver; frames are forwarded as JSON text on a 50 ms cadence.

use std::time::Duration;

use actix::{Actor, AsyncContext, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws::{self, Message, ProtocolError, WebsocketContext};

use crate::v1::dmn_metrics::{DmnMetricsSnapshot, DmnMetricsTx};

/// Cadence for polling the broadcast channel and forwarding buffered frames.
const BROADCAST_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// WebSocket session actor streaming `DmnMetricsSnapshot` frames to a connected client.
struct DmnMetricsWsSession {
    dmn_metrics_tx: DmnMetricsTx,
}

impl Actor for DmnMetricsWsSession {
    type Context = WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let mut rx = self.dmn_metrics_tx.subscribe();
        ctx.run_interval(BROADCAST_POLL_INTERVAL, move |_this: &mut Self, ctx: &mut WebsocketContext<Self>| {
            while let Ok(snapshot) = rx.try_recv() {
                let payload = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string());
                ctx.text(payload);
            }
        });
    }
}

impl StreamHandler<Result<Message, ProtocolError>> for DmnMetricsWsSession {
    fn handle(
        &mut self,
        item: Result<Message, ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match item {
            Ok(Message::Ping(msg)) => ctx.pong(&msg),
            Ok(Message::Close(reason)) => ctx.close(reason),
            Ok(_) => (), // read-only broadcast: ignore Text/Binary/Pong/Continuation
            Err(_) => {} // protocol error: drop the frame, let the runtime handle it
        }
    }
}

/// WebSocket upgrade handler for `/v1/metrics/dmn/ws`.
pub async fn dmn_metrics_ws(
    req: HttpRequest,
    stream: web::Payload,
    dmn_metrics_tx: web::Data<DmnMetricsTx>,
) -> Result<HttpResponse, actix_web::Error> {
    ws::start(
        DmnMetricsWsSession {
            dmn_metrics_tx: dmn_metrics_tx.get_ref().clone(),
        },
        &req,
        stream,
    )
}

/// Configure the DMN metrics WebSocket endpoint inside the `/v1` scope.
pub fn configure_dmn_metrics_ws(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.route("/metrics/dmn/ws", web::get().to(dmn_metrics_ws));
}
