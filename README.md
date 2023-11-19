[![Docs.rs](https://docs.rs/hyper-tungstenite/badge.svg)](https://docs.rs/crate/hyper-tungstenite/)
[![CI](https://github.com/de-vri-es/hyper-tungstenite-rs/workflows/CI/badge.svg)](https://github.com/de-vri-es/hyper-tungstenite-rs/actions?query=workflow%3ACI+branch%3Amain)

# hyper-tungstenite

This crate allows [`hyper`](https://docs.rs/hyper) servers to accept websocket connections, backed by [`tungstenite`](https://docs.rs/tungstenite).

The [`upgrade`] function allows you to upgrade a HTTP connection to a websocket connection.
It returns a HTTP response to send to the client, and a future that resolves to a [`WebSocketStream`].
The response must be sent to the client for the future to be resolved.
In practise this means that you must spawn the future in a different task.

Note that the [`upgrade`] function itself does not check if the request is actually an upgrade request.
For simple cases, you can check this using the [`is_upgrade_request`] function before calling [`upgrade`].
For more complicated cases where the server should support multiple upgrade protocols,
you can manually inspect the `Connection` and `Upgrade` headers.

## Example
```rust
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};
use hyper_tungstenite::{tungstenite, HyperWebsocket};
use hyper_util::rt::TokioIo;
use tungstenite::Message;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Handle a HTTP or WebSocket request.
async fn handle_request(mut request: Request<Incoming>) -> Result<Response<Full<Bytes>>, Error> {
    // Check if the request is a websocket upgrade request.
    if hyper_tungstenite::is_upgrade_request(&request) {
        let (response, websocket) = hyper_tungstenite::upgrade(&mut request, None)?;

        // Spawn a task to handle the websocket connection.
        tokio::spawn(async move {
            if let Err(e) = serve_websocket(websocket).await {
                eprintln!("Error in websocket connection: {e}");
            }
        });

        // Return the response so the spawned future can continue.
        Ok(response)
    } else {
        // Handle regular HTTP requests here.
        Ok(Response::new(Full::<Bytes>::from("Hello HTTP!")))
    }
}

/// Handle a websocket connection.
async fn serve_websocket(websocket: HyperWebsocket) -> Result<(), Error> {
    let mut websocket = websocket.await?;
    while let Some(message) = websocket.next().await {
        match message? {
            Message::Text(msg) => {
                println!("Received text message: {msg}");
                websocket.send(Message::text("Thank you, come again.")).await?;
            },
            Message::Binary(msg) => {
                println!("Received binary message: {msg:02X?}");
                websocket.send(Message::binary(b"Thank you, come again.".to_vec())).await?;
            },
            Message::Ping(msg) => {
                // No need to send a reply: tungstenite takes care of this for you.
                println!("Received ping message: {msg:02X?}");
            },
            Message::Pong(msg) => {
                println!("Received pong message: {msg:02X?}");
            }
            Message::Close(msg) => {
                // No need to send a reply: tungstenite takes care of this for you.
                if let Some(msg) = &msg {
                    println!("Received close message with code {} and message: {}", msg.code, msg.reason);
                } else {
                    println!("Received close message");
                }
            },
            Message::Frame(_msg) => {
                unreachable!();
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let addr: std::net::SocketAddr = "[::1]:3000".parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Listening on http://{addr}");

    let mut http = hyper::server::conn::http1::Builder::new();
    http.keep_alive(true);

    loop {
        let (stream, _) = listener.accept().await?;
        let connection = http
            .serve_connection(TokioIo::new(stream), hyper::service::service_fn(handle_request))
            .with_upgrades();
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                println!("Error serving HTTP connection: {err:?}");
            }
        });
    }
}
```

[`upgrade`]: https://docs.rs/hyper-tungstenite/latest/hyper_tungstenite/fn.upgrade.html
[`WebSocketStream`]: https://docs.rs/hyper-tungstenite/latest/hyper_tungstenite/struct.WebSocketStream.html
[`is_upgrade_request`]: https://docs.rs/hyper-tungstenite/latest/hyper_tungstenite/fn.is_upgrade_request.html
