use http_body_util::Full;
use hyper::{Request, Response};
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use hyper_tungstenite::tungstenite::Error;
use tokio::net::TcpStream;
use std::net::Ipv6Addr;
use tokio_tungstenite::tungstenite::{Message, Result};
use futures::stream::StreamExt;
use futures::sink::SinkExt;

use assert2::{assert, let_assert};

#[tokio::test]
async fn hyper() {
	// Bind a TCP listener to an ephemeral port.
	let_assert!(Ok(listener) = tokio::net::TcpListener::bind((Ipv6Addr::LOCALHOST, 0u16)).await);
	let_assert!(Ok(bind_addr) = listener.local_addr());
	let server = hyper::server::conn::http1::Builder::new();

	// Spawn the server in a task.
	tokio::spawn(async move {
		let service = service_fn(upgrade_websocket);
		let_assert!(Ok((stream, _)) = listener.accept().await);
		let_assert!(Ok(()) = server.serve_connection(TokioIo::new(stream), service).with_upgrades().await);
	});

	// Try to create a websocket connection with the server.
	let_assert!(Ok(stream) = TcpStream::connect(bind_addr).await);
	let_assert!(Ok((mut stream, _response)) = tokio_tungstenite::client_async("ws://localhost/foo", stream).await);

	let_assert!(Some(Ok(message)) = stream.next().await);
	assert!(message == Message::text("Hello!"));

	let_assert!(Ok(()) = stream.send(Message::text("Goodbye!")).await);
	assert!(let Some(Ok(Message::Close(None))) = stream.next().await);
}

async fn upgrade_websocket(mut request: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
	assert!(hyper_tungstenite::is_upgrade_request(&request) == true);

	let (response, stream) = hyper_tungstenite::upgrade(&mut request, None)
		.map_err(Error::Protocol)?;
	tokio::spawn(async move {
		let_assert!(Ok(mut stream) = stream.await);
		assert!(let Ok(()) = stream.send(Message::text("Hello!")).await);
		let_assert!(Some(Ok(reply)) = stream.next().await);
		assert!(reply == Message::text("Goodbye!"));
		assert!(let Ok(()) = stream.send(Message::Close(None)).await);
	});

	Ok(response)
}
