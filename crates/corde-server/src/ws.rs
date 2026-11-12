//! `GET /ws` — one task per dashboard.
//!
//! Sequence: `Hello` → then `Frame` at the client's rate (default `--rate`,
//! `ClientMessage::SetRate` 1..=60 Hz; each tick reads the latest frame from
//! the `watch` and sends it only if its `seq` differs from the last one sent),
//! `Lap` / `Event` as they happen, `Status` on change and every 5 s, `Pong`
//! for every `Ping`. JSON text frames only; `?format=msgpack` → 501 for now.

use crate::config::MAX_RATE_HZ;
use crate::state::{AppState, BusMessage, SERVER_VERSION};
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use corde_protocol::{ClientMessage, ServerMessage, PROTOCOL_VERSION};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::{interval, MissedTickBehavior};

#[derive(Debug, Deserialize, Default)]
pub struct WsQuery {
    /// `json` (default). `msgpack` is reserved for a later version.
    pub format: Option<String>,
    /// Initial broadcast rate, Hz (same bounds as `SetRate`).
    pub rate: Option<u8>,
}

pub async fn ws_handler(
    State(state): State<Arc<AppState>>,
    Query(q): Query<WsQuery>,
    device: Option<axum::Extension<crate::hub::auth::DeviceCtx>>,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    // Format is checked first so that a plain GET /ws?format=msgpack answers 501.
    match q
        .format
        .as_deref()
        .map(|f| f.to_ascii_lowercase())
        .as_deref()
    {
        None | Some("json") => {}
        Some("msgpack") | Some("messagepack") => {
            // TODO(msgpack): same schema, binary frames.
            return (
                StatusCode::NOT_IMPLEMENTED,
                Json(
                    serde_json::json!({ "error": "msgpack not implemented yet; use ?format=json" }),
                ),
            )
                .into_response();
        }
        Some(other) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("unknown format {other:?}") })),
            )
                .into_response();
        }
    }
    let ws = match ws {
        Ok(ws) => ws,
        Err(rejection) => return rejection.into_response(),
    };
    let rate = q.rate.map(clamp_rate).unwrap_or(state.default_rate);
    // Home server: the socket closes as soon as its device is revoked.
    let revocation = match (&state.hub, device) {
        (Some(hub), Some(axum::Extension(d))) => Some((d.id, hub.auth.subscribe_revocations())),
        _ => None,
    };
    // The iPhone application asks for `corde.v1` (plus, on the home server, its device token
    // as a second entry, read by the hub guard): the answer names `corde.v1` only — never the
    // token entry. A browser that asks for nothing gets no subprotocol, as before.
    ws.protocols([crate::hub::auth::WS_PROTOCOL])
        .max_message_size(crate::guard::WS_MAX_MESSAGE_BYTES)
        .max_frame_size(crate::guard::WS_MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| client_loop(socket, state, rate, revocation))
}

/// Resolves when `device` is revoked (never without a device).
async fn revoked(rev: &mut Option<(String, broadcast::Receiver<String>)>) {
    match rev {
        Some((device, rx)) => loop {
            match rx.recv().await {
                Ok(id) if id == *device => return,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => std::future::pending::<()>().await,
            }
        },
        None => std::future::pending::<()>().await,
    }
}

fn clamp_rate(hz: u8) -> u8 {
    hz.clamp(1, MAX_RATE_HZ)
}

fn period(hz: u8) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(clamp_rate(hz)))
}

fn frame_ticker(hz: u8) -> tokio::time::Interval {
    let mut t = interval(period(hz));
    t.set_missed_tick_behavior(MissedTickBehavior::Delay);
    t
}

async fn send_json(socket: &mut WebSocket, msg: &ServerMessage) -> bool {
    match serde_json::to_string(msg) {
        Ok(s) => socket.send(Message::Text(Utf8Bytes::from(s))).await.is_ok(),
        Err(e) => {
            tracing::warn!("ws: cannot serialise {e}");
            true
        }
    }
}

async fn client_loop(
    mut socket: WebSocket,
    state: Arc<AppState>,
    initial_rate: u8,
    mut revocation: Option<(String, broadcast::Receiver<String>)>,
) {
    let _guard = state.client_connected();
    let mut rate = initial_rate;
    let mut shutdown = state.shutdown.subscribe();
    let mut bus = state.bus.subscribe();
    let frame_rx = state.frame_rx.clone();

    let hello = ServerMessage::Hello {
        server_version: SERVER_VERSION.to_string(),
        protocol_version: PROTOCOL_VERSION,
        status: state.status(rate as f32),
    };
    if !send_json(&mut socket, &hello).await {
        return;
    }

    let mut ticker = frame_ticker(rate);
    let mut status_timer = interval(Duration::from_secs(5));
    status_timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
    status_timer.reset(); // first tick 5 s from now, Hello already carried the status
    let mut last_seq: Option<u64> = None;
    let mut sent_frames: u64 = 0;

    loop {
        tokio::select! {
            biased;

            _ = shutdown.changed() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }

            _ = revoked(&mut revocation) => {
                tracing::info!("ws: device revoked, connection closed");
                let _ = socket
                    .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                        code: 4401,
                        reason: Utf8Bytes::from_static("device revoked"),
                    })))
                    .await;
                break;
            }

            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMessage>(text.as_str()) {
                            Ok(ClientMessage::SetRate { hz }) => {
                                let new_rate = clamp_rate(hz);
                                if new_rate != rate {
                                    rate = new_rate;
                                    ticker = frame_ticker(rate);
                                    tracing::debug!("ws: rate → {rate} Hz");
                                }
                                if !send_json(&mut socket, &ServerMessage::Status(state.status(rate as f32))).await {
                                    break;
                                }
                            }
                            Ok(ClientMessage::Ping { t }) => {
                                if !send_json(&mut socket, &ServerMessage::Pong { t }).await {
                                    break;
                                }
                            }
                            Err(e) => tracing::debug!("ws: ignoring client message ({e}): {}", text.as_str().chars().take(120).collect::<String>()),
                        }
                    }
                    Some(Ok(Message::Binary(_))) => tracing::debug!("ws: binary client messages are not supported yet"),
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        tracing::debug!("ws: closed with error: {e}");
                        break;
                    }
                }
            }

            _ = ticker.tick() => {
                let slot = frame_rx.borrow().clone();
                if let Some(slot) = slot {
                    if last_seq != Some(slot.frame.seq) {
                        last_seq = Some(slot.frame.seq);
                        sent_frames += 1;
                        if socket.send(Message::Text(Utf8Bytes::from(slot.json().to_string()))).await.is_err() {
                            break;
                        }
                    }
                }
            }

            msg = bus.recv() => {
                match msg {
                    Ok(BusMessage::Json(json)) => {
                        if socket.send(Message::Text(Utf8Bytes::from(json.to_string()))).await.is_err() {
                            break;
                        }
                    }
                    Ok(BusMessage::StatusChanged) => {
                        if !send_json(&mut socket, &ServerMessage::Status(state.status(rate as f32))).await {
                            break;
                        }
                        status_timer.reset();
                    }
                    Err(RecvError::Lagged(n)) => tracing::debug!("ws: client lagged, {n} bus messages skipped"),
                    Err(RecvError::Closed) => break,
                }
            }

            _ = status_timer.tick() => {
                if !send_json(&mut socket, &ServerMessage::Status(state.status(rate as f32))).await {
                    break;
                }
            }
        }
    }
    tracing::debug!("ws: client gone after {sent_frames} frames");
}
