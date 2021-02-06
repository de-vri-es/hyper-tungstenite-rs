use hyper::{Body, Request, Response};
use hyper::server::Server;
use hyper::service::{service_fn, make_service_fn};
use tokio::net::TcpStream;
use std::net::Ipv6Addr;
use tokio_tungstenite::tungstenite::{Message, Result};
use futures::stream::StreamExt;
use futures::sink::SinkExt;

use assert2::{assert, let_assert};

#[tokio::test]
async fn hyper() {
	// Bind a TCP listener to an ephemeral port.
	let_assert!(Ok(listener) = std::net::TcpListener::bind((Ipv6Addr::LOCALHOST, 0u16)));
	let_assert!(Ok(bind_addr) = listener.local_addr());
	let_assert!(Ok(server) = Server::from_tcp(listener));

	// Spawn the server in a task.
	tokio::spawn(async move {
		let service = make_service_fn(|_conn| async {
			Ok::<_, hyper::Error>(service_fn(upgrade_websocket))
		});
		let_assert!(Ok(()) = server.http1_only(true).serve(service).await);
	});

	// Try to create a websocket connection with the server.
	let_assert!(Ok(stream) = TcpStream::connect(bind_addr).await);
	let_assert!(Ok((mut stream, _response)) = tokio_tungstenite::client_async("ws://localhost/foo", stream).await);

	let_assert!(Some(Ok(message)) = stream.next().await);
	assert!(message == Message::text("Hello!"));

	let_assert!(Ok(()) = stream.send(Message::text("Goodbye!")).await);
	assert!(let Some(Ok(Message::Close(None))) = stream.next().await);
}

async fn upgrade_websocket(request: Request<Body>) -> Result<Response<Body>> {
	assert!(hyper_tungstenite::is_upgrade_request(&request) == true);

	let (response, stream) = hyper_tungstenite::upgrade(request, None)?;
	tokio::spawn(async move {
		let_assert!(Ok(mut stream) = stream.await);
		assert!(let Ok(()) = stream.send(Message::text("Hello!")).await);
		let_assert!(Some(Ok(reply)) = stream.next().await);
		assert!(reply == Message::text("Goodbye!"));
		assert!(let Ok(()) = stream.send(Message::Close(None)).await);
	});

	Ok(response)
}
