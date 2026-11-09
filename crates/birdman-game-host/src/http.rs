//! Same-listener HTTP assets, bootstrap and raw WebSocket transport.

use crate::service::{HostListenConfig, HostServiceHandle};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{
        State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use birdman_game_runtime::{
    APPLICATION_NETWORK_BINARY_BYTES, APPLICATION_NETWORK_PROTOCOL,
    APPLICATION_NETWORK_SERVER_TEXT_BYTES, ApplicationNetworkMessage, ApplicationNetworkPayload,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    net::{SocketAddr, TcpListener},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone)]
struct HttpState {
    config: HostListenConfig,
    host: HostServiceHandle,
    active: Arc<AtomicBool>,
}

/// A listener guard; dropping it closes HTTP requests and network peer delivery.
pub(crate) struct HostListener {
    pub address: SocketAddr,
    pub https: bool,
    active: Arc<AtomicBool>,
    server: axum_server::Handle,
    task: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl HostListener {
    pub async fn start(config: HostListenConfig, host: HostServiceHandle) -> Result<Self, String> {
        let tls = if let Some(tls) = &config.tls {
            Some(
                axum_server::tls_rustls::RustlsConfig::from_pem_file(
                    &tls.certificate_chain_pem,
                    &tls.private_key_pem,
                )
                .await
                .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        let listener = TcpListener::bind(config.bind).map_err(|error| error.to_string())?;
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let address = listener.local_addr().map_err(|error| error.to_string())?;
        let https = tls.is_some();
        let active = Arc::new(AtomicBool::new(true));
        let router = Router::new()
            .route("/api/host", get(bootstrap))
            .route("/api/session/ws", get(upgrade))
            .fallback(asset)
            .with_state(HttpState {
                config,
                host,
                active: active.clone(),
            });
        let server = axum_server::Handle::new();
        let handle = server.clone();
        let task = match tls {
            Some(tls) => {
                let server = axum_server::from_tcp_rustls(listener, tls)
                    .map_err(|error| error.to_string())?;
                tokio::spawn(async move {
                    server
                        .handle(handle)
                        .serve(router.into_make_service())
                        .await
                })
            }
            None => {
                let server = axum_server::from_tcp(listener).map_err(|error| error.to_string())?;
                tokio::spawn(async move {
                    server
                        .handle(handle)
                        .serve(router.into_make_service())
                        .await
                })
            }
        };
        Ok(Self {
            address,
            https,
            server,
            active,
            task: Some(task),
        })
    }

    pub fn is_finished(&self) -> bool {
        self.task.as_ref().is_some_and(|task| task.is_finished())
    }

    pub async fn terminal_cause(&mut self) -> String {
        match self.task.take().expect("terminal listener task").await {
            Ok(Err(error)) => error.to_string(),
            Err(error) => error.to_string(),
            Ok(Ok(())) => "HTTP listener terminated".to_owned(),
        }
    }

    pub async fn stop(mut self) -> Result<(), String> {
        self.active.store(false, Ordering::Release);
        self.server.shutdown();
        let Some(task) = self.task.take() else {
            return Ok(());
        };
        task.abort();
        match task.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) if error.is_cancelled() => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

impl Drop for HostListener {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        self.server.shutdown();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

async fn bootstrap(State(state): State<HttpState>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        state.host.bootstrap_json.to_string(),
    )
        .into_response()
}

async fn asset(State(state): State<HttpState>, method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let Some(asset) = state.config.web.asset(uri.path()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let length = asset.bytes.len();
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(Bytes::from_owner(asset.bytes))
    };
    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        asset.content_type.parse().expect("fixed MIME"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        asset.cache_control.parse().expect("fixed cache policy"),
    );
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, length.into());
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        "nosniff".parse().expect("fixed header"),
    );
    response
}

async fn upgrade(
    State(state): State<HttpState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !same_origin(&headers, state.config.tls.is_some()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.protocols([APPLICATION_NETWORK_PROTOCOL])
        .max_message_size(
            APPLICATION_NETWORK_SERVER_TEXT_BYTES.max(APPLICATION_NETWORK_BINARY_BYTES),
        )
        .max_frame_size(APPLICATION_NETWORK_SERVER_TEXT_BYTES.max(APPLICATION_NETWORK_BINARY_BYTES))
        .on_upgrade(move |socket| connection(socket, state.host, state.active))
        .into_response()
}

fn same_origin(headers: &HeaderMap, https: bool) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|header| header.to_str().ok())
    else {
        return false;
    };
    let Ok(origin) = origin
        .to_str()
        .ok()
        .and_then(|value| url::Url::parse(value).ok())
        .ok_or(())
    else {
        return false;
    };
    let scheme = if https { "https" } else { "http" };
    let Ok(expected) = url::Url::parse(&format!("{scheme}://{host}/")) else {
        return false;
    };
    origin.scheme() == scheme
        && expected.username().is_empty()
        && expected.password().is_none()
        && expected.path() == "/"
        && expected.query().is_none()
        && expected.fragment().is_none()
        && origin.username().is_empty()
        && origin.password().is_none()
        && origin.path() == "/"
        && origin.query().is_none()
        && origin.fragment().is_none()
        && origin.host() == expected.host()
        && origin.port_or_known_default() == expected.port_or_known_default()
}

async fn connection(socket: WebSocket, host: HostServiceHandle, active: Arc<AtomicBool>) {
    let Ok(mut peer) = host.connect(Some(active)).await else {
        return;
    };
    let (mut sink, mut stream) = socket.split();
    loop {
        tokio::select! {
            _ = &mut peer.disconnected => break,
            incoming = stream.next() => {
                let message = match incoming {
                    Some(Ok(Message::Text(text))) if text.len() <= APPLICATION_NETWORK_SERVER_TEXT_BYTES => ApplicationNetworkMessage::Text(text.to_string()),
                    Some(Ok(Message::Binary(bytes))) if bytes.len() <= APPLICATION_NETWORK_BINARY_BYTES => ApplicationNetworkMessage::Binary(bytes.to_vec()),
                    Some(Ok(Message::Ping(_)|Message::Pong(_))) => continue,
                    _ => break,
                };
                if host.receive(peer.id, message).await.is_err() { break; }
            }
            outgoing = peer.transmissions.recv() => {
                let Some(outgoing) = outgoing else { break; };
                let close = matches!(outgoing.payload, ApplicationNetworkPayload::Close { .. });
                let message = match outgoing.payload {
                    ApplicationNetworkPayload::Text(text) => Message::Text(text.to_string().into()),
                    ApplicationNetworkPayload::Binary(bytes) => Message::Binary(Bytes::from_owner(bytes)),
                    ApplicationNetworkPayload::Close { reason } => {
                        let end = reason.char_indices().map(|(index, _)| index).take_while(|index| *index <= 123).last().unwrap_or(0);
                        let reason = if reason.len() <= 123 { reason } else { reason[..end].to_owned() };
                        Message::Close(Some(CloseFrame { code: 1000, reason: reason.into() }))
                    }
                };
                let delivery = tokio::select! {
                    _ = &mut peer.disconnected => Err("WebSocket peer retired".into()),
                    result = tokio::time::timeout(Duration::from_secs(5), sink.send(message)) => Ok(result),
                };
                let result = match delivery {
                    Err(error) => Err(error),
                    Ok(result) => match result {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(_) => Err("WebSocket write timed out".into()),
                    }
                };
                let failed = result.is_err();
                if host.transmitted(peer.id, outgoing.id, result).await.is_err() || failed || close { break; }
            }
        }
    }
    host.disconnect(peer.id).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_uses_scheme_host_and_effective_port() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "localhost:8080".parse().unwrap());
        assert!(same_origin(&headers, false));
        for (origin, accepted) in [
            ("http://localhost:8080", true),
            ("https://localhost:8080", false),
            ("http://localhost:8080.attacker.invalid", false),
            ("null", false),
            ("http://localhost:8081", false),
        ] {
            headers.insert(header::ORIGIN, origin.parse().unwrap());
            assert_eq!(same_origin(&headers, false), accepted, "{origin}");
        }
        headers.insert(header::ORIGIN, "http://localhost:8080".parse().unwrap());
        for host in ["user@localhost:8080", "localhost:8080/path"] {
            headers.insert(header::HOST, host.parse().unwrap());
            assert!(!same_origin(&headers, false), "{host}");
        }
    }
}
