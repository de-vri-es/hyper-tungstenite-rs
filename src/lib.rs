//! This crate allows [`hyper`](https://docs.rs/hyper) servers to accept websocket connections, backed by [`tungstenite`](https://docs.rs/tungstenite).
//!
//! The [`upgrade`] function allows you to upgrade a HTTP connection to a websocket connection.
//! It returns a HTTP response to send to the client, and a future that resolves to a [`WebSocketStream`].
//! The response must be sent to the client for the future to be resolved.
//! In practise this means that you must spawn the future in a different task.
//!
//! Note that the [`upgrade`] function itself does not check if the request is actually an upgrade request.
//! For simple cases, you can check this using the [`upgrade_requested`] function before calling [`upgrade`].
//! For more complicated cases where the server should support multiple upgrade protocols,
//! you can manually inspect the `Connection` and `Upgrade` headers.
//!
//! # Example
//! ```no_run
//! use futures::{sink::SinkExt, stream::StreamExt};
//! use hyper::{Body, Request, Response};
//! use hyper_tungstenite::{tungstenite, HyperWebsocket};
//! use tungstenite::Message;
//! # fn foo(message: &Message) {}
//!
//! /// Handle a HTTP or WebSocket request.
//! async fn handle_request(request: Request<Body>) -> Result<Response<Body>, Box<dyn std::error::Error>> {
//!     // Check if the request is a websocket upgrade request.
//!     if hyper_tungstenite::upgrade_requested(&request) {
//!         let (response, websocket) = hyper_tungstenite::upgrade(request, None)?;
//!
//!         // Spawn a task to handle the websocket connection.
//!         tokio::spawn(async move {
//!             if let Err(e) = serve_websocket(websocket).await {
//!                 eprintln!("Error in websocket connection: {}", e);
//!             }
//!         });
//!
//!         // Return the response so the spawned future can continue.
//!         Ok(response)
//!     } else {
//!         // Handle regular HTTP requests here.
//!         Ok(Response::new(Body::from("Hello HTTP!")))
//!     }
//! }
//!
//! /// Handle a websocket connection.
//! async fn serve_websocket(websocket: HyperWebsocket) -> Result<(), Box<dyn std::error::Error>> {
//!     let mut websocket = websocket.await?;
//!     while let Some(message) = websocket.next().await {
//!         let message = message?;
//!
//!         // Do something with the message.
//!         foo(&message);
//!
//!         // Send a reply.
//!         websocket.send(Message::text("Thank you, come again.")).await?;
//!     }
//!
//!     Ok(())
//! }
//! ```

use hyper::{Body, Request, Response};
use std::task::{Context, Poll};
use std::pin::Pin;

use tungstenite::{Error, Result};
use tungstenite::protocol::{Role, WebSocketConfig};

pub use tokio_tungstenite::tungstenite;
pub use tokio_tungstenite::WebSocketStream;

/// A future that resolves to a websocket stream when the associated HTTP upgrade completes.
pub struct HyperWebsocket {
	inner: hyper::upgrade::OnUpgrade,
	config: Option<WebSocketConfig>,
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
/// To check if a request is a websocket upgrade request, you can use [`upgrade_requested`].
/// Alternatively you can inspect the `Connection` and `Upgrade` headers manually.
///
pub fn upgrade(request: Request<Body>, config: Option<WebSocketConfig>) -> Result<(Response<Body>, HyperWebsocket)> {
	let key = request.headers().get("Sec-WebSocket-Key")
		.ok_or_else(|| protocol_error("missing \"Sec-WebSocket-Key\" header"))?;
	let version = request.headers().get("Sec-WebSocket-Version")
		.ok_or_else(|| protocol_error("missing \"Sec-WebSocket-Version\" header"))?;
	if version.as_bytes() != b"13" {
		return Err(protocol_error(format!("invalid websocket protocol version: expected 13, got {:?}", version)));
	}

	let response = Response::builder()
		.status(hyper::StatusCode::SWITCHING_PROTOCOLS)
		.header(hyper::header::CONNECTION, "upgrade")
		.header(hyper::header::UPGRADE, "websocket")
		.header("Sec-WebSocket-Accept", &convert_key(key.as_bytes()))
		.body(Body::from("switching to websocket protocol"))?;

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
pub fn upgrade_requested<B>(request: &hyper::Request<B>) -> bool {
	// Check for "Connection: upgrade" header.
	if let Some(connection) = request.headers().get(hyper::header::CONNECTION) {
		if !connection.as_bytes().eq_ignore_ascii_case(b"upgrade") {
			return false;
		}
	}

	// Check for "Upgrade: websocket" header.
	if let Some(upgrade) = request.headers().get(hyper::header::UPGRADE) {
		if let Ok(upgrade) = upgrade.to_str() {
			if upgrade.split(',').any(|x| x.trim().eq_ignore_ascii_case("websocket")) {
				return true;
			}
		}
	}

	false
}

fn protocol_error(message: impl Into<std::borrow::Cow<'static, str>>) -> Error {
	tungstenite::Error::Protocol(message.into())
}

/// Turns a Sec-WebSocket-Key into a Sec-WebSocket-Accept.
fn convert_key(input: &[u8]) -> String {
	use sha1::Digest;

	// ... field is constructed by concatenating /key/ ...
	// ... with the string "258EAFA5-E914-47DA-95CA-C5AB0DC85B11" (RFC 6455)
	const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
	let mut sha1 = sha1::Sha1::default();
	sha1.update(input);
	sha1.update(WS_GUID);
	base64::encode(sha1.finalize())
}

impl std::future::Future for HyperWebsocket {
	type Output = Result<WebSocketStream<hyper::upgrade::Upgraded>>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
		let inner = unsafe { self.as_mut().map_unchecked_mut(|x| &mut x.inner) };
		let upgraded = match inner.poll(cx) {
			Poll::Pending => return Poll::Pending,
			Poll::Ready(x) => x,
		};

		let upgraded = upgraded.map_err(|e| protocol_error(format!("failed to upgrade HTTP connection: {}", e)))?;

		let mut stream = WebSocketStream::from_raw_socket(
			upgraded,
			Role::Server,
			self.config.take(),
		);
		let stream = unsafe { Pin::new_unchecked(&mut stream) };

		// The future returned by `from_raw_socket` is always ready.
		// Not sure why it is a future in the first place.
		match stream.poll(cx) {
			Poll::Pending => unreachable!("from_raw_socket should always be created ready"),
			Poll::Ready(x) => Poll::Ready(Ok(x)),
		}
	}
}
