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
use futures::{sink::SinkExt, stream::StreamExt};
use hyper::{Body, Request, Response};
use hyper_tungstenite::{tungstenite, HyperWebsocket};
use tungstenite::Message;

/// Handle a HTTP or WebSocket request.
async fn handle_request(request: Request<Body>) -> Result<Response<Body>, Box<dyn std::error::Error>> {
    // Check if the request is a websocket upgrade request.
    if hyper_tungstenite::is_upgrade_request(&request) {
        let (response, websocket) = hyper_tungstenite::upgrade(request, None)?;

        // Spawn a task to handle the websocket connection.
        tokio::spawn(async move {
            if let Err(e) = serve_websocket(websocket).await {
                eprintln!("Error in websocket connection: {}", e);
            }
        });

        // Return the response so the spawned future can continue.
        Ok(response)
    } else {
        // Handle regular HTTP requests here.
        Ok(Response::new(Body::from("Hello HTTP!")))
    }
}

/// Handle a websocket connection.
async fn serve_websocket(websocket: HyperWebsocket) -> Result<(), Box<dyn std::error::Error>> {
    let mut websocket = websocket.await?;
    while let Some(message) = websocket.next().await {
        let message = message?;

        // Do something with the message.
        foo(&message);

        // Send a reply.
        websocket.send(Message::text("Thank you, come again.")).await?;
    }

    Ok(())
}
```

[`upgrade`]: https://docs.rs/hyper-tungstenite/latest/hyper_tungstenite/fn.upgrade.html
[`WebSocketStream`]: https://docs.rs/hyper-tungstenite/latest/hyper_tungstenite/struct.WebSocketStream.html
[`is_upgrade_request`]: https://docs.rs/hyper-tungstenite/latest/hyper_tungstenite/fn.is_upgrade_request.html
