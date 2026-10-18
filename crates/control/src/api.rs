//! Network API: HTTP and WebSocket control of Prism from another computer
//! or a tablet (ADR 0035).
//!
//! Commands use the fixed OSC addresses (`/prism/layer/1/clip/2`…) and
//! become [`ControlMessage`]s from an OSC source, so they follow the same
//! rules and learned mappings as OSC. Every route but the page asks for the
//! access code.

use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use prism_core::{Composition, ControlSource};
use serde::Deserialize;
use tokio::sync::{oneshot, watch};

use crate::ControlMessage;

/// Port the API listens on by default.
pub const DEFAULT_API_PORT: u16 = 8080;

/// Characters of access codes: no 0/O or 1/I/L, easy to type.
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

/// Length of access codes.
const CODE_LENGTH: usize = 6;

/// The control page for tablets.
const PAGE: &str = include_str!("api_page.html");

/// A new random access code.
pub fn access_code() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut code = String::with_capacity(CODE_LENGTH);
    for index in 0..CODE_LENGTH {
        // Each `RandomState` is seeded by the system at random.
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_usize(index);
        let pick = hasher.finish() as usize % CODE_ALPHABET.len();
        code.push(char::from(CODE_ALPHABET[pick]));
    }
    code
}

/// What the API shows of `composition`. Arrays are in interface order;
/// addresses number layers and columns from 1.
pub fn api_state(composition: &Composition) -> serde_json::Value {
    let columns: Vec<_> = composition
        .columns()
        .iter()
        .map(|column| serde_json::json!({ "name": column.name }))
        .collect();
    let layers: Vec<_> = composition
        .layers()
        .iter()
        .map(|layer| {
            let active = layer.active_column().and_then(|active| {
                composition
                    .columns()
                    .iter()
                    .position(|column| column.id() == active)
            });
            let clips: Vec<_> = layer
                .clips()
                .iter()
                .map(|clip| clip.as_ref().map(|clip| clip.name.clone()))
                .collect();
            serde_json::json!({
                "name": layer.name,
                "opacity": layer.opacity.get(),
                "bypass": layer.bypass,
                "solo": layer.solo,
                "active": active,
                "clips": clips,
            })
        })
        .collect();
    serde_json::json!({
        "tempo": composition.tempo.get(),
        "crossfader": composition.crossfader.position.get(),
        "columns": columns,
        "layers": layers,
    })
}

/// A command: an address of Prism and, for a fader, its value from 0 to 1.
/// Without a value, the command is a press and release of a button.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Command {
    /// Address, as in OSC: `/prism/column/2`.
    pub address: String,
    /// Value from 0 to 1.
    #[serde(default)]
    pub value: Option<f32>,
}

impl Command {
    /// The messages of the command, or `None` for an address outside
    /// `/prism/`.
    pub fn messages(&self) -> Option<Vec<ControlMessage>> {
        if !self.address.starts_with("/prism/") {
            return None;
        }
        let source = ControlSource::Osc {
            address: self.address.clone(),
        };
        let message = |value: f32| ControlMessage {
            source: source.clone(),
            value,
            pressed: value >= 0.5,
        };
        Some(match self.value {
            Some(value) => vec![message(value.clamp(0.0, 1.0))],
            None => vec![message(1.0), message(0.0)],
        })
    }
}

/// Shared by the routes.
struct Shared {
    code: String,
    sender: std::sync::Mutex<Sender<ControlMessage>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    state: watch::Receiver<String>,
}

impl Shared {
    fn authorized(&self, headers: &HeaderMap, query: &CodeQuery) -> bool {
        let bearer = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "));
        bearer
            .or(query.code.as_deref())
            .is_some_and(|given| same_code(given.trim(), &self.code))
    }

    fn send(&self, command: &Command) -> bool {
        let Some(messages) = command.messages() else {
            return false;
        };
        if let Ok(sender) = self.sender.lock() {
            for message in messages {
                let _ = sender.send(message);
            }
        }
        (self.wake)();
        true
    }
}

/// Compares codes in a time that does not depend on where they differ.
fn same_code(given: &str, code: &str) -> bool {
    let given = given.to_ascii_uppercase();
    given.len() == code.len()
        && given
            .bytes()
            .zip(code.bytes())
            .fold(0u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

#[derive(Debug, Default, Deserialize)]
struct CodeQuery {
    code: Option<String>,
}

/// The API server, running on its own thread until dropped.
pub struct ApiServer {
    port: u16,
    state: Option<watch::Sender<String>>,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl ApiServer {
    /// Listens on `port` of every network interface. Commands go to
    /// `sender`, then `wake` is called.
    pub fn start(
        port: u16,
        code: String,
        sender: Sender<ControlMessage>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let (state, state_receiver) = watch::channel(String::from("{}"));
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let shared = Arc::new(Shared {
            code,
            sender: std::sync::Mutex::new(sender),
            wake,
            state: state_receiver,
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?;
        let thread = std::thread::Builder::new()
            .name("prism-api".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
                        return;
                    };
                    let _ = axum::serve(listener, router(shared))
                        .with_graceful_shutdown(async {
                            let _ = shutdown_receiver.await;
                        })
                        .await;
                });
            })?;
        Ok(Self {
            port,
            state: Some(state),
            shutdown: Some(shutdown),
            thread: Some(thread),
        })
    }

    /// Port listened on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Makes `state` the state clients read; WebSocket clients get it when
    /// it changed.
    pub fn publish(&self, state: &serde_json::Value) {
        let text = state.to_string();
        if let Some(sender) = &self.state {
            sender.send_if_modified(|current| {
                if *current == text {
                    false
                } else {
                    *current = text;
                    true
                }
            });
        }
    }
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        // Closing the state ends the WebSocket connections.
        self.state = None;
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn router(shared: Arc<Shared>) -> Router {
    Router::new()
        .route("/", get(|| async { Html(PAGE) }))
        .route("/api/state", get(state))
        .route("/api/control", post(control))
        .route("/api/ws", get(socket))
        .with_state(shared)
}

fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "code d'accès manquant ou faux").into_response()
}

async fn state(
    State(shared): State<Arc<Shared>>,
    Query(query): Query<CodeQuery>,
    headers: HeaderMap,
) -> Response {
    if !shared.authorized(&headers, &query) {
        return unauthorized();
    }
    let body = shared.state.borrow().clone();
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn control(
    State(shared): State<Arc<Shared>>,
    Query(query): Query<CodeQuery>,
    headers: HeaderMap,
    axum::Json(command): axum::Json<Command>,
) -> Response {
    if !shared.authorized(&headers, &query) {
        return unauthorized();
    }
    if shared.send(&command) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (StatusCode::BAD_REQUEST, "adresse hors de /prism/").into_response()
    }
}

async fn socket(
    State(shared): State<Arc<Shared>>,
    Query(query): Query<CodeQuery>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !shared.authorized(&headers, &query) {
        return unauthorized();
    }
    upgrade.on_upgrade(move |socket| serve_socket(socket, shared))
}

/// Sends the state at once and on each change; reads commands.
async fn serve_socket(mut socket: WebSocket, shared: Arc<Shared>) {
    let mut state = shared.state.clone();
    let first = state.borrow_and_update().clone();
    if socket.send(Message::Text(first.into())).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            changed = state.changed() => {
                if changed.is_err() {
                    break;
                }
                let text = state.borrow_and_update().clone();
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            received = socket.recv() => {
                match received {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(command) = serde_json::from_str::<Command>(&text) {
                            shared.send(&command);
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    #[test]
    fn codes_are_six_easy_characters() {
        let code = access_code();
        assert_eq!(code.len(), CODE_LENGTH);
        assert!(code.bytes().all(|byte| CODE_ALPHABET.contains(&byte)));
        assert_ne!(access_code(), access_code() + "x");
    }

    #[test]
    fn codes_compare_whole_and_ignore_case() {
        assert!(same_code("abc234", "ABC234"));
        assert!(!same_code("ABC23", "ABC234"));
        assert!(!same_code("ABC235", "ABC234"));
    }

    #[test]
    fn a_command_without_value_is_a_press_and_release() {
        let command = Command {
            address: "/prism/column/2".into(),
            value: None,
        };
        let messages = command.messages().unwrap();
        assert_eq!(messages.len(), 2);
        assert!(messages[0].pressed && !messages[1].pressed);
        let fader = Command {
            address: "/prism/crossfader".into(),
            value: Some(1.5),
        };
        assert_eq!(fader.messages().unwrap()[0].value, 1.0);
        let outside = Command {
            address: "/other".into(),
            value: None,
        };
        assert!(outside.messages().is_none());
    }

    #[test]
    fn the_state_lists_layers_columns_and_clips() {
        let composition = Composition::default();
        let state = api_state(&composition);
        assert_eq!(
            state["layers"].as_array().unwrap().len(),
            composition.layers().len()
        );
        assert_eq!(
            state["columns"].as_array().unwrap().len(),
            composition.columns().len()
        );
        assert!(state["crossfader"].is_number());
    }

    /// Sends a raw HTTP request, returns the status line and the body.
    fn request(port: u16, text: &str) -> (String, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(text.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let status = response.lines().next().unwrap_or_default().to_owned();
        let body = response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_owned())
            .unwrap_or_default();
        (status, body)
    }

    #[test]
    fn routes_ask_for_the_code_and_pass_commands() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let server = ApiServer::start(0, "ABC234".into(), sender, Arc::new(|| {})).unwrap();
        server.publish(&serde_json::json!({ "tempo": 120 }));
        let port = server.port();

        let (status, body) = request(port, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n");
        assert!(status.contains("200"), "{status}");
        assert!(body.contains("Prism"));

        let (status, _) = request(port, "GET /api/state HTTP/1.1\r\nConnection: close\r\n\r\n");
        assert!(status.contains("401"), "{status}");

        let (status, body) = request(
            port,
            "GET /api/state?code=abc234 HTTP/1.1\r\nConnection: close\r\n\r\n",
        );
        assert!(status.contains("200"), "{status}");
        assert_eq!(body, r#"{"tempo":120}"#);

        let json = r#"{"address":"/prism/layer/1/clip/2"}"#;
        let (status, _) = request(
            port,
            &format!(
                "POST /api/control HTTP/1.1\r\nConnection: close\r\nAuthorization: Bearer ABC234\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{json}",
                json.len()
            ),
        );
        assert!(status.contains("204"), "{status}");
        let message = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(
            message.source,
            ControlSource::Osc {
                address: "/prism/layer/1/clip/2".into()
            }
        );
        assert!(message.pressed);
        drop(server);
    }
}
