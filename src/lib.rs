//! This crate allows [`hyper`](https://docs.rs/hyper) servers to accept websocket connections, backed by [`tungstenite`](https://docs.rs/tungstenite).
//!
//! The [`upgrade`] function allows you to upgrade a HTTP connection to a websocket connection.
//! It returns a HTTP response to send to the client, and a future that resolves to a [`WebSocketStream`].
//! The response must be sent to the client for the future to be resolved.
//! In practise this means that you must spawn the future in a different task.
//!
//! Note that the [`upgrade`] function itself does not check if the request is actually an upgrade request.
//! For simple cases, you can check this using the [`is_upgrade_request`] function before calling [`upgrade`].
//! For more complicated cases where the server should support multiple upgrade protocols,
//! you can manually inspect the `Connection` and `Upgrade` headers.
//!
//! # Example
//! ```no_run
//! use futures::sink::SinkExt;
//! use futures::stream::StreamExt;
//! use http_body_util::Full;
//! use hyper::body::{Bytes, Incoming};
//! use hyper::{Request, Response};
//! use hyper_tungstenite::{tungstenite, HyperWebsocket};
//! use hyper_util::rt::TokioIo;
//! use tungstenite::Message;
//!
//! type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
//!
//! /// Handle a HTTP or WebSocket request.
//! async fn handle_request(mut request: Request<Incoming>) -> Result<Response<Full<Bytes>>, Error> {
//!     // Check if the request is a websocket upgrade request.
//!     if hyper_tungstenite::is_upgrade_request(&request) {
//!         let (response, websocket) = hyper_tungstenite::upgrade(&mut request, None)?;
//!
//!         // Spawn a task to handle the websocket connection.
//!         tokio::spawn(async move {
//!             if let Err(e) = serve_websocket(websocket).await {
//!                 eprintln!("Error in websocket connection: {e}");
//!             }
//!         });
//!
//!         // Return the response so the spawned future can continue.
//!         Ok(response)
//!     } else {
//!         // Handle regular HTTP requests here.
//!         Ok(Response::new(Full::<Bytes>::from("Hello HTTP!")))
//!     }
//! }
//!
//! /// Handle a websocket connection.
//! async fn serve_websocket(websocket: HyperWebsocket) -> Result<(), Error> {
//!     let mut websocket = websocket.await?;
//!     while let Some(message) = websocket.next().await {
//!         match message? {
//!             Message::Text(msg) => {
//!                 println!("Received text message: {msg}");
//!                 websocket.send(Message::text("Thank you, come again.")).await?;
//!             },
//!             Message::Binary(msg) => {
//!                 println!("Received binary message: {msg:02X?}");
//!                 websocket.send(Message::binary(b"Thank you, come again.".to_vec())).await?;
//!             },
//!             Message::Ping(msg) => {
//!                 // No need to send a reply: tungstenite takes care of this for you.
//!                 println!("Received ping message: {msg:02X?}");
//!             },
//!             Message::Pong(msg) => {
//!                 println!("Received pong message: {msg:02X?}");
//!             }
//!             Message::Close(msg) => {
//!                 // No need to send a reply: tungstenite takes care of this for you.
//!                 if let Some(msg) = &msg {
//!                     println!("Received close message with code {} and message: {}", msg.code, msg.reason);
//!                 } else {
//!                     println!("Received close message");
//!                 }
//!             },
//!             Message::Frame(_msg) => {
//!                 unreachable!();
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let addr: std::net::SocketAddr = "[::1]:3000".parse()?;
//!     let listener = tokio::net::TcpListener::bind(&addr).await?;
//!     println!("Listening on http://{addr}");
//!
//!     let mut http = hyper::server::conn::http1::Builder::new();
//!     http.keep_alive(true);
//!
//!     loop {
//!         let (stream, _) = listener.accept().await?;
//!         let connection = http
//!             .serve_connection(TokioIo::new(stream), hyper::service::service_fn(handle_request))
//!             .with_upgrades();
//!         tokio::spawn(async move {
//!             if let Err(err) = connection.await {
//!                 println!("Error serving HTTP connection: {err:?}");
//!             }
//!         });
//!     }
//! }
//! ```

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::task::{Context, Poll};
use std::pin::Pin;
use pin_project_lite::pin_project;

use tungstenite::{Error, error::ProtocolError};
use tungstenite::handshake::derive_accept_key;
use tungstenite::protocol::{Role, WebSocketConfig};

pub use hyper;
pub use tungstenite;

pub use tokio_tungstenite::WebSocketStream;

/// A [`WebSocketStream`] that wraps an upgraded HTTP connection from hyper.
pub type HyperWebsocketStream = WebSocketStream<TokioIo<hyper::upgrade::Upgraded>>;

pin_project! {
	/// A future that resolves to a websocket stream when the associated HTTP upgrade completes.
	#[derive(Debug)]
	pub struct HyperWebsocket {
		#[pin]
		inner: hyper::upgrade::OnUpgrade,
		config: Option<WebSocketConfig>,
	}
}

/// Try to upgrade a received `hyper::Request` to a websocket connection.
///
/// The function returns a HTTP response and a future that resolves to the websocket stream.
/// The response body *MUST* be sent to the client before the future can be resolved.
///
/// This functions checks `Sec-WebSocket-Key` and `Sec-WebSocket-Version` headers.
/// It does not inspect the `Origin`, `Sec-WebSocket-Protocol` or `Sec-WebSocket-Extensions` headers.
/// You can inspect the headers manually before calling this function,
/// and modify the response headers appropriately.
///
/// This function also does not look at the `Connection` or `Upgrade` headers.
/// To check if a request is a websocket upgrade request, you can use [`is_upgrade_request`].
/// Alternatively you can inspect the `Connection` and `Upgrade` headers manually.
///
pub fn upgrade<B>(
	mut request: impl std::borrow::BorrowMut<Request<B>>,
	config: Option<WebSocketConfig>,
) -> Result<(Response<Full<Bytes>>, HyperWebsocket), ProtocolError> {
	let request = request.borrow_mut();

	let key = request.headers().get("Sec-WebSocket-Key")
		.ok_or(ProtocolError::MissingSecWebSocketKey)?;
	if request.headers().get("Sec-WebSocket-Version").map(|v| v.as_bytes()) != Some(b"13") {
		return Err(ProtocolError::MissingSecWebSocketVersionHeader);
	}

	let response = Response::builder()
		.status(hyper::StatusCode::SWITCHING_PROTOCOLS)
		.header(hyper::header::CONNECTION, "upgrade")
		.header(hyper::header::UPGRADE, "websocket")
		.header("Sec-WebSocket-Accept", &derive_accept_key(key.as_bytes()))
		.body(Full::<Bytes>::from("switching to websocket protocol"))
		.expect("bug: failed to build response");

	let stream = HyperWebsocket {
		inner: hyper::upgrade::on(request),
		config,
	};

	Ok((response, stream))
}

/// Check if a request is a websocket upgrade request.
///
/// If the `Upgrade` header lists multiple protocols,
/// this function returns true if of them are `"websocket"`,
/// If the server supports multiple upgrade protocols,
/// it would be more appropriate to try each listed protocol in order.
pub fn is_upgrade_request<B>(request: &hyper::Request<B>) -> bool {
	header_contains_value(request.headers(), hyper::header::CONNECTION, "Upgrade")
		&& header_contains_value(request.headers(), hyper::header::UPGRADE, "websocket")
}

/// Check if there is a header of the given name containing the wanted value.
fn header_contains_value(headers: &hyper::HeaderMap, header: impl hyper::header::AsHeaderName, value: impl AsRef<[u8]>) -> bool {
	let value = value.as_ref();
	for header in headers.get_all(header) {
		if header.as_bytes().split(|&c| c == b',').any(|x| trim(x).eq_ignore_ascii_case(value)) {
			return true;
		}
	}
	false
}

fn trim(data: &[u8]) -> &[u8] {
	trim_end(trim_start(data))
}

fn trim_start(data: &[u8]) -> &[u8] {
	if let Some(start) =data.iter().position(|x| !x.is_ascii_whitespace()) {
		&data[start..]
	} else {
		b""
	}
}

fn trim_end(data: &[u8]) -> &[u8] {
	if let Some(last) = data.iter().rposition(|x| !x.is_ascii_whitespace()) {
		&data[..last + 1]
	} else {
		b""
	}
}

impl std::future::Future for HyperWebsocket {
	type Output = Result<HyperWebsocketStream, Error>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
		let this = self.project();
		let upgraded = match this.inner.poll(cx) {
			Poll::Pending => return Poll::Pending,
			Poll::Ready(x) => x,
		};

		let upgraded = upgraded.map_err(|_| Error::Protocol(ProtocolError::HandshakeIncomplete))?;

		let stream = WebSocketStream::from_raw_socket(
			TokioIo::new(upgraded),
			Role::Server,
			this.config.take(),
		);
		tokio::pin!(stream);

		// The future returned by `from_raw_socket` is always ready.
		// Not sure why it is a future in the first place.
		match stream.as_mut().poll(cx) {
			Poll::Pending => unreachable!("from_raw_socket should always be created ready"),
			Poll::Ready(x) => Poll::Ready(Ok(x)),
		}
	}
}
