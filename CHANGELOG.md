# v0.14.0 - 2024-06-03
* Update `tungstenite` and `tokio-tungstenite` to `v0.23`.

# v0.13.0 - 2023-12-11
* Update `tungstenite` and `tokio-tungstenite` to `v0.21`.

# v0.12.0 - 2023-11-19
* Update `hyper` to version `1.0`.
* Add public `HyperWebsocketStream` type alias.

# v0.11.1 - 2023-07-31
* Replace `pin-project` with `pin-project-lite`.

# v0.11.0 - 2023-07-31
* Update `tungstenite` and `tokio-tungstenite` to `v0.20`.

# v0.10.0 - 2023-05-17
* Update `tungstenite` and `tokio-tungstenite` to `v0.19`.

# v0.9.0 - 2022-12-03
* Update `tungstenite` and `tokio-tungstenite` to `v0.18`.

# v0.8.2 - 2022-11-15
* Update the example to avoid soon-deprecated `hyper` types.

# v0.8.1 - 2022-06-14
* Fix the documentation for `tungstenite` 0.17.

# v0.8.0 - 2022-02-26
* Allow arbitrary body types in the `Request` passed to `upgrade`.

# v0.7.0 - 2022-02-25
* Accept either a `Request` or `&mut Request` when upgrading a connection.

# v0.6.0 - 2022-02-20
* Update to `tungstenite` 0.17.

# v0.5.0 - 2021-11-19
* Update to `tungstenite` 0.16.

# v0.4.2 - 2021-11-19
* Fix link in documentation for re-exported `tungstenite` crate.

# v0.4.1 - 2021-10-17
* Update the example to a full server application.

# v0.4.0 - 2021-08-28
* Upgrade to `tokio-tungstenite` 0.15.

# v0.3.3 - 2021-06-11
* Remove `sha-1` and `base64` dependency by using upstream `derive_accept_key`.

# v0.3.2 - 2021-04-11
* Derive `Debug` for `HyperWebsocket` to facilitate debugging.

# v0.3.1 - 2021-04-03
* Replace unsafe code with `pin-project` and `tokio::pin!()`.

# v0.3.0 - 2021-03-02
* Publicly re-export the `hyper` crate.
* Upgrade to `tokio-tungstenite` 0.14 and `tungstenite` 0.13.

# v0.2.1 - 2021-02-12
* Inspect all `Connection` and `Upgrade` headers in `is_upgrade_request()`.
* Inspect all comma separated values in `Connection` headers in `is_upgrade_request()` (this was already done for `Upgrade` headers).

# v0.2.0 - 2021-02-06
* Rename `upgrade_requested` to `is_upgrade_request`.

# v0.1.1 - 2021-02-06
* Fix category slug in Cargo manifest.

# v0.1.0 - 2021-02-06
* Initial release.
