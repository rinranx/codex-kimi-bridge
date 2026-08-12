use crate::error::{BridgeError, BridgeResult, sanitize_provider_error};
use crate::protocol::{StreamTranslator, translate_chat_completion, translate_responses_request};
use crate::reasoning::ReasoningStore;
use crate::sse::{SseDecoder, encode_done, encode_event};
use crate::{DEFAULT_UPSTREAM, VERSION};
use async_stream::stream;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::header::{
    ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONNECTION, CONTENT_TYPE, HeaderName, HeaderValue,
};
use axum::http::{Method, Request, Response, StatusCode};
use axum::response::IntoResponse;
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::redirect::Policy;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

const DEFAULT_TIMEOUT_MS: u64 = 7_200_000;
const DEFAULT_MAX_BODY_BYTES: usize = 128 * 1024 * 1024;
const MAX_UPSTREAM_ERROR_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub upstream: String,
    pub model: String,
    pub timeout_ms: u64,
    pub max_body_bytes: usize,
    pub allow_non_loopback: bool,
    pub allow_insecure_upstream: bool,
    pub quiet: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8787,
            upstream: DEFAULT_UPSTREAM.into(),
            model: "k3".into(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            allow_non_loopback: false,
            allow_insecure_upstream: false,
            quiet: false,
        }
    }
}

#[derive(Clone)]
struct AppState {
    config: Arc<NormalizedConfig>,
    client: reqwest::Client,
    reasoning_store: Arc<ReasoningStore>,
}

#[derive(Clone, Debug)]
struct NormalizedConfig {
    upstream: reqwest::Url,
    model: String,
    max_body_bytes: usize,
    quiet: bool,
}

pub fn build_router(config: ServerConfig) -> BridgeResult<Router> {
    let normalized = Arc::new(normalize_config(&config)?);
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_millis(config.timeout_ms.max(1)))
        .build()
        .map_err(|_| {
            BridgeError::new("The Rust HTTPS client could not be initialized.")
                .status(500)
                .kind("bridge_error")
                .code("http_client_initialization_failed")
        })?;
    let state = AppState {
        config: normalized,
        client,
        reasoning_store: Arc::new(ReasoningStore::new()),
    };
    Ok(Router::new().fallback(handle_request).with_state(state))
}

pub async fn serve(config: ServerConfig) -> BridgeResult<()> {
    let router = build_router(config.clone())?;
    let listener = TcpListener::bind((config.host.as_str(), config.port))
        .await
        .map_err(|error| {
            BridgeError::new(format!(
                "Could not listen on {}:{}: {error}",
                config.host, config.port
            ))
            .status(500)
            .kind("bridge_error")
            .code("bind_failed")
        })?;
    let address = listener.local_addr().map_err(|error| {
        BridgeError::new(format!("Could not resolve the listening address: {error}"))
            .status(500)
            .kind("bridge_error")
            .code("bind_failed")
    })?;
    if !config.quiet {
        eprintln!(
            "codex-kimi-bridge {VERSION} listening on http://{}:{}",
            config.host,
            address.port()
        );
        let upstream = reqwest::Url::parse(&config.upstream).map_err(|_| {
            BridgeError::new("The upstream URL is invalid.").code("invalid_upstream_url")
        })?;
        let origin = format!(
            "{}://{}{}",
            upstream.scheme(),
            upstream.host_str().unwrap_or("unknown"),
            upstream
                .port()
                .map(|port| format!(":{port}"))
                .unwrap_or_default()
        );
        eprintln!("upstream: {origin}");
        eprintln!("implementation: rust");
        eprintln!("privacy: request bodies and credentials are not logged");
    }
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|error| {
            BridgeError::new(format!("The bridge server stopped unexpectedly: {error}"))
                .status(500)
                .kind("bridge_error")
                .code("server_failed")
        })
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn handle_request(State(state): State<AppState>, request: Request<Body>) -> Response<Body> {
    let result = handle_inner(state.clone(), request).await;
    let mut response = match result {
        Ok(response) => response,
        Err(error) => {
            if !state.config.quiet {
                eprintln!(
                    "{}",
                    json!({
                        "event": "request_failed",
                        "status": error.status,
                        "code": error.code,
                    })
                );
            }
            json_response(status_code(error.status), error.envelope())
        }
    };
    let headers = response.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );
    response
}

async fn handle_inner(state: AppState, request: Request<Body>) -> BridgeResult<Response<Body>> {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    if method == Method::GET && matches!(path.as_str(), "/" | "/health" | "/v1/health") {
        return Ok(json_response(
            StatusCode::OK,
            json!({
                "status": "ok",
                "service": "codex-kimi-bridge",
                "implementation": "rust",
                "version": VERSION,
                "model": state.config.model,
                "upstream": redacted_upstream(&state.config.upstream),
                "logging": "request bodies and credentials are not logged",
            }),
        ));
    }
    if method == Method::GET && path == "/v1/models" {
        return Ok(json_response(
            StatusCode::OK,
            json!({
                "object": "list",
                "data": [{
                    "id": state.config.model,
                    "object": "model",
                    "created": 0,
                    "owned_by": "kimi-code",
                }]
            }),
        ));
    }
    if method != Method::POST || !matches!(path.as_str(), "/v1/responses" | "/responses") {
        return Ok(json_response(
            StatusCode::NOT_FOUND,
            json!({
                "error": {
                    "message": "Not found. Use POST /v1/responses or GET /health.",
                    "type": "invalid_request_error",
                    "code": "not_found",
                    "param": Value::Null,
                }
            }),
        ));
    }

    let authorization = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with("Bearer ") && value.len() > 7)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            BridgeError::new(
                "A Bearer token is required. Codex should supply it through the provider auth command.",
            )
            .status(401)
            .kind("authentication_error")
            .code("missing_api_key")
        })?;
    let body = to_bytes(request.into_body(), state.config.max_body_bytes)
        .await
        .map_err(|_| {
            BridgeError::new(format!(
                "Request body exceeds the {}-byte limit.",
                state.config.max_body_bytes
            ))
            .status(413)
            .code("request_too_large")
        })?;
    let parsed: Value = serde_json::from_slice(&body)
        .map_err(|_| BridgeError::new("Request body must be valid JSON.").code("invalid_json"))?;
    let translated = translate_responses_request(
        parsed,
        &state.config.model,
        Some(state.reasoning_store.clone()),
    )?;
    let streaming = translated.body.get("stream").and_then(Value::as_bool) == Some(true);
    let upstream = state
        .client
        .post(state.config.upstream.clone())
        .header(AUTHORIZATION, authorization)
        .header(CONTENT_TYPE, "application/json")
        .header(
            ACCEPT,
            if streaming {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .header("user-agent", format!("codex-kimi-bridge/{VERSION}"))
        .body(serde_json::to_vec(&translated.body).map_err(|_| {
            BridgeError::new("The translated request could not be serialized.")
                .status(500)
                .kind("bridge_error")
                .code("serialization_failed")
        })?)
        .send()
        .await
        .map_err(map_reqwest_error)?;

    let status = upstream.status();
    if !status.is_success() {
        let bytes = read_limited(upstream, MAX_UPSTREAM_ERROR_BYTES).await?;
        let parsed = serde_json::from_slice::<Value>(&bytes).unwrap_or_else(|_| {
            json!({
                "error": {
                    "message": String::from_utf8_lossy(&bytes),
                }
            })
        });
        return Ok(json_response(
            status_code(status.as_u16()),
            sanitize_provider_error(&parsed, status.as_u16()),
        ));
    }

    if streaming {
        let mut upstream_stream = upstream.bytes_stream();
        let context = translated.context;
        let output = stream! {
            let mut decoder = SseDecoder::default();
            let mut translator = StreamTranslator::new(context);
            let mut failed = false;
            let mut upstream_done = false;
            'upstream: while let Some(chunk) = upstream_stream.next().await {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(error) => {
                        let bridge_error = map_reqwest_error(error);
                        yield Ok::<Bytes, Infallible>(Bytes::from(encode_event(&bridge_error.stream_event(u64::MAX))));
                        failed = true;
                        break;
                    }
                };
                let frames = match decoder.push(&chunk) {
                    Ok(frames) => frames,
                    Err(error) => {
                        yield Ok(Bytes::from(encode_event(&error.stream_event(u64::MAX))));
                        failed = true;
                        break;
                    }
                };
                for frame in frames {
                    if frame.data == "[DONE]" {
                        upstream_done = true;
                        break 'upstream;
                    }
                    let chunk: Value = match serde_json::from_str(&frame.data) {
                        Ok(chunk) => chunk,
                        Err(_) => {
                            let error = BridgeError::new("The upstream SSE stream contained invalid JSON.")
                                .status(502)
                                .kind("upstream_protocol_error")
                                .code("invalid_upstream_sse");
                            yield Ok(Bytes::from(encode_event(&error.stream_event(u64::MAX))));
                            failed = true;
                            break 'upstream;
                        }
                    };
                    match translator.ingest(&chunk) {
                        Ok(events) => {
                            for event in events {
                                yield Ok(Bytes::from(encode_event(&event)));
                            }
                        }
                        Err(error) => {
                            yield Ok(Bytes::from(encode_event(&error.stream_event(u64::MAX))));
                            failed = true;
                            break 'upstream;
                        }
                    }
                }
            }
            if !failed && !upstream_done {
                match decoder.finish() {
                    Ok(frames) => {
                        for frame in frames {
                            if frame.data == "[DONE]" {
                                break;
                            }
                            match serde_json::from_str::<Value>(&frame.data) {
                                Ok(chunk) => match translator.ingest(&chunk) {
                                    Ok(events) => for event in events {
                                        yield Ok(Bytes::from(encode_event(&event)));
                                    },
                                    Err(error) => {
                                        yield Ok(Bytes::from(encode_event(&error.stream_event(u64::MAX))));
                                        failed = true;
                                        break;
                                    }
                                },
                                Err(_) => {
                                    let error = BridgeError::new("The upstream SSE stream contained invalid JSON.")
                                        .status(502)
                                        .kind("upstream_protocol_error")
                                        .code("invalid_upstream_sse");
                                    yield Ok(Bytes::from(encode_event(&error.stream_event(u64::MAX))));
                                    failed = true;
                                    break;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        yield Ok(Bytes::from(encode_event(&error.stream_event(u64::MAX))));
                        failed = true;
                    }
                }
            }
            if !failed {
                for event in translator.finish() {
                    yield Ok(Bytes::from(encode_event(&event)));
                }
                yield Ok(Bytes::from_static(encode_done()));
            }
        };
        let mut response = Response::new(Body::from_stream(output));
        *response.status_mut() = StatusCode::OK;
        let headers = response.headers_mut();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream; charset=utf-8"),
        );
        headers.insert(
            CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-transform"),
        );
        headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
        headers.insert(
            HeaderName::from_static("x-accel-buffering"),
            HeaderValue::from_static("no"),
        );
        return Ok(response);
    }

    let bytes = read_limited(upstream, state.config.max_body_bytes).await?;
    let chat: Value = serde_json::from_slice(&bytes).map_err(|_| {
        BridgeError::new("The upstream response was not valid JSON.")
            .status(502)
            .kind("upstream_protocol_error")
            .code("invalid_upstream_response")
    })?;
    let response = translate_chat_completion(&chat, &translated.context)?;
    Ok(json_response(StatusCode::OK, response))
}

fn normalize_config(config: &ServerConfig) -> BridgeResult<NormalizedConfig> {
    if !is_loopback(&config.host) && !config.allow_non_loopback {
        return Err(BridgeError::new(
            "Refusing to bind outside loopback. Pass --allow-non-loopback only if you understand that API tokens will cross that interface.",
        )
        .code("unsafe_bind_address"));
    }
    let upstream = reqwest::Url::parse(&config.upstream).map_err(|_| {
        BridgeError::new("The upstream URL is invalid.").code("invalid_upstream_url")
    })?;
    if !upstream.username().is_empty() || upstream.password().is_some() {
        return Err(
            BridgeError::new("The upstream URL must not contain embedded credentials.")
                .code("unsafe_upstream_url"),
        );
    }
    let upstream_host = upstream.host_str().unwrap_or("");
    if upstream.scheme() != "https"
        && !(config.allow_insecure_upstream && is_loopback(upstream_host))
    {
        return Err(BridgeError::new(
            "The upstream URL must use HTTPS. Plain HTTP is allowed only for an explicit loopback test server.",
        )
        .code("unsafe_upstream_url"));
    }
    Ok(NormalizedConfig {
        upstream,
        model: if config.model.trim().is_empty() {
            "k3".into()
        } else {
            config.model.clone()
        },
        max_body_bytes: config.max_body_bytes.max(1),
        quiet: config.quiet,
    })
}

fn map_reqwest_error(error: reqwest::Error) -> BridgeError {
    if error.is_timeout() {
        BridgeError::new("Upstream request timed out.")
            .status(504)
            .kind("timeout_error")
            .code("request_aborted")
    } else {
        BridgeError::new("The bridge could not reach the configured upstream.")
            .status(502)
            .kind("upstream_connection_error")
            .code("upstream_connection_failed")
    }
}

async fn read_limited(response: reqwest::Response, limit: usize) -> BridgeResult<Vec<u8>> {
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(map_reqwest_error)?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(BridgeError::new(
                "The upstream response exceeded the configured byte limit.",
            )
            .status(502)
            .kind("upstream_protocol_error")
            .code("upstream_response_too_large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn json_response(status: StatusCode, value: Value) -> Response<Body> {
    let mut response = axum::Json(value).into_response();
    *response.status_mut() = status;
    response
}

fn status_code(value: u16) -> StatusCode {
    StatusCode::from_u16(value).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

fn redacted_upstream(url: &reqwest::Url) -> String {
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!(
        "{}://{}{}{}",
        url.scheme(),
        url.host_str().unwrap_or("unknown"),
        port,
        url.path()
    )
}

pub fn is_loopback(host: &str) -> bool {
    if matches!(host, "localhost" | "127.0.0.1" | "::1") {
        return true;
    }
    host.parse::<IpAddr>()
        .is_ok_and(|address| address.is_loopback())
}

pub async fn is_port_available(host: &str, port: u16) -> bool {
    match TcpListener::bind((host, port)).await {
        Ok(listener) => {
            drop(listener);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use std::collections::VecDeque;
    use tokio::sync::Mutex;
    use tokio::task::JoinHandle;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct MockUpstreamState {
        replies: Arc<Mutex<VecDeque<MockReply>>>,
        captured: Arc<Mutex<Vec<CapturedRequest>>>,
    }

    struct MockReply {
        status: StatusCode,
        content_type: &'static str,
        body: String,
    }

    #[derive(Debug)]
    struct CapturedRequest {
        authorization: Option<String>,
        user_agent: Option<String>,
        body: Value,
    }

    async fn mock_upstream_handler(
        State(state): State<MockUpstreamState>,
        request: Request<Body>,
    ) -> Response<Body> {
        let authorization = request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let user_agent = request
            .headers()
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let bytes = to_bytes(request.into_body(), 16 * 1024 * 1024)
            .await
            .unwrap();
        let body = serde_json::from_slice(&bytes).unwrap();
        state.captured.lock().await.push(CapturedRequest {
            authorization,
            user_agent,
            body,
        });
        let reply = state.replies.lock().await.pop_front().unwrap();
        Response::builder()
            .status(reply.status)
            .header(CONTENT_TYPE, reply.content_type)
            .body(Body::from(reply.body))
            .unwrap()
    }

    async fn spawn_mock_upstream(
        replies: Vec<MockReply>,
    ) -> (String, Arc<Mutex<Vec<CapturedRequest>>>, JoinHandle<()>) {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let state = MockUpstreamState {
            replies: Arc::new(Mutex::new(replies.into())),
            captured: captured.clone(),
        };
        let app = Router::new()
            .fallback(mock_upstream_handler)
            .with_state(state);
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}/chat"), captured, task)
    }

    fn bridge_for(upstream: String) -> Router {
        build_router(ServerConfig {
            upstream,
            allow_insecure_upstream: true,
            quiet: true,
            ..ServerConfig::default()
        })
        .unwrap()
    }

    async fn response_json(response: Response<Body>) -> Value {
        serde_json::from_slice(
            &to_bytes(response.into_body(), 16 * 1024 * 1024)
                .await
                .unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn health_is_public_and_generation_requires_auth() {
        let app = build_router(ServerConfig::default()).unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(body["service"], "codex-kimi-bridge");
        assert_eq!(body["implementation"], "rust");

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/responses")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"model":"k3","input":"Hi"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn bridges_non_streaming_request_without_exposing_credentials() {
        let (upstream, captured, task) = spawn_mock_upstream(vec![MockReply {
            status: StatusCode::OK,
            content_type: "application/json",
            body: json!({
                "id": "chatcmpl_mock",
                "created": 123,
                "model": "k3",
                "choices": [{
                    "index": 0,
                    "finish_reason": "stop",
                    "message": { "role": "assistant", "content": "KIMI_BRIDGE_OK" }
                }],
                "usage": { "prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5 }
            })
            .to_string(),
        }])
        .await;
        let app = bridge_for(upstream);
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/responses")
                    .header(AUTHORIZATION, "Bearer super-secret-test-token")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "model": "k3",
                            "input": "PRIVATE_PROMPT_MARKER",
                            "reasoning": { "effort": "low" },
                            "stream": false
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let result = response_json(response).await;
        assert_eq!(result["output"][0]["content"][0]["text"], "KIMI_BRIDGE_OK");
        let captured = captured.lock().await;
        assert_eq!(
            captured[0].authorization.as_deref(),
            Some("Bearer super-secret-test-token")
        );
        assert_eq!(
            captured[0].user_agent.as_deref(),
            Some("codex-kimi-bridge/0.3.0")
        );
        assert_eq!(
            captured[0].body["messages"][0]["content"],
            "PRIVATE_PROMPT_MARKER"
        );
        assert_eq!(captured[0].body["reasoning_effort"], "low");
        drop(captured);
        task.abort();
    }

    #[tokio::test]
    async fn bridges_kimi_sse_to_responses_sse() {
        let upstream_sse = [
            json!({
                "id": "chatcmpl_sse",
                "created": 100,
                "model": "k3",
                "choices": [{ "index": 0, "delta": { "content": "Hello" }, "finish_reason": Value::Null }]
            }),
            json!({
                "id": "chatcmpl_sse",
                "model": "k3",
                "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
                "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
            }),
        ]
        .into_iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect::<String>()
            + "data: [DONE]\n\n";
        let (upstream, _, task) = spawn_mock_upstream(vec![MockReply {
            status: StatusCode::OK,
            content_type: "text/event-stream",
            body: upstream_sse,
        }])
        .await;
        let response = bridge_for(upstream)
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/responses")
                    .header(AUTHORIZATION, "Bearer test-token")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({ "model": "k3", "input": "Hi", "stream": true }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let text = String::from_utf8(
            to_bytes(response.into_body(), 16 * 1024 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(text.contains("event: response.created"));
        assert!(text.contains("event: response.output_text.delta"));
        assert!(text.contains("event: response.completed"));
        assert!(text.contains("data: [DONE]"));
        task.abort();
    }

    #[tokio::test]
    async fn preserves_reasoning_across_server_round_trip() {
        let replies = vec![
            MockReply {
                status: StatusCode::OK,
                content_type: "application/json",
                body: json!({
                    "id": "chatcmpl_first",
                    "model": "k3",
                    "choices": [{
                        "finish_reason": "tool_calls",
                        "message": {
                            "role": "assistant",
                            "content": Value::Null,
                            "reasoning_content": "private tool reasoning",
                            "tool_calls": [{
                                "id": "call_roundtrip",
                                "type": "function",
                                "function": { "name": "read_file", "arguments": "{\"path\":\"a.txt\"}" }
                            }]
                        }
                    }]
                })
                .to_string(),
            },
            MockReply {
                status: StatusCode::OK,
                content_type: "application/json",
                body: json!({
                    "id": "chatcmpl_second",
                    "model": "k3",
                    "choices": [{
                        "finish_reason": "stop",
                        "message": { "role": "assistant", "content": "done" }
                    }]
                })
                .to_string(),
            },
        ];
        let (upstream, captured, task) = spawn_mock_upstream(replies).await;
        let bridge = bridge_for(upstream);
        let first = bridge
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/responses")
                    .header(AUTHORIZATION, "Bearer test-token")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "model": "k3",
                            "input": "Read a.txt",
                            "tools": [{ "type": "function", "name": "read_file", "parameters": { "type": "object" } }],
                            "stream": false
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response_json(first).await["output"][0]["call_id"],
            "call_roundtrip"
        );
        let second = bridge
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/responses")
                    .header(AUTHORIZATION, "Bearer test-token")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "model": "k3",
                            "input": [
                                { "role": "user", "content": "Read a.txt" },
                                { "type": "function_call", "call_id": "call_roundtrip", "name": "read_file", "arguments": "{\"path\":\"a.txt\"}" },
                                { "type": "function_call_output", "call_id": "call_roundtrip", "output": "contents" }
                            ],
                            "stream": false
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::OK);
        let captured = captured.lock().await;
        assert_eq!(
            captured[1].body["messages"][1]["reasoning_content"],
            "private tool reasoning"
        );
        drop(captured);
        task.abort();
    }

    #[test]
    fn refuses_unsafe_network_defaults() {
        let config = ServerConfig {
            host: "0.0.0.0".into(),
            ..ServerConfig::default()
        };
        assert_eq!(
            build_router(config).err().unwrap().code,
            "unsafe_bind_address"
        );
        let config = ServerConfig {
            upstream: "http://example.com/v1/chat/completions".into(),
            ..ServerConfig::default()
        };
        assert_eq!(
            build_router(config).err().unwrap().code,
            "unsafe_upstream_url"
        );
    }
}
