# my-tcp-sockets

Async TCP server/client building blocks for Tokio with pluggable serialization, ping/pong health checks, reconnecting clients, and optional TLS or Unix Domain Sockets.

## Features
- Event-driven callbacks for connect/disconnect/payload handling.
- Your own protocol via `TcpSocketSerializer` + `TcpSerializerState`.
- Built-in ping/pong (`TcpContract::is_pong`) and send helpers (`send`, `send_many`, `send_bytes`, `send_ping`).
- Client auto-reconnect, configurable timeouts, and per-connection statistics.
- Optional TLS support with `with-tls` feature; Unix Domain Sockets on Unix.

## Add to Cargo.toml
```toml
[dependencies]
my-tcp-sockets = { git = "https://github.com/MyJetTools/my-tcp-sockets.git", tag = "0.1.12" }
# Enable TLS if needed
# my-tcp-sockets = { git = "https://github.com/MyJetTools/my-tcp-sockets.git", tag = "0.1.12", features = ["with-tls"] }
```

## Core concepts
- **`TcpContract`**: Your message type. The trait has a single method — `is_pong(&self) -> bool` — so the keep-alive loop can spot pong replies. **There is no `is_ping()`** on this trait; outgoing pings are produced by the serializer's `get_ping()`.
- **`TcpSocketSerializer`**: Serializes contracts to bytes and deserializes from a socket reader. Provides `get_ping()` for keep-alive.
- **`TcpSerializerState`**: Per-connection state that can be updated by incoming contracts. Use `is_tcp_contract_related_to_metadata()` to filter which contracts update state, and `apply_tcp_contract()` to mutate it.
- **`TcpSerializerFactory`**: Creates serializer and state instances for each new connection.
- **`SocketEventCallback`**: Async hooks for `connected()`, `payload()`, `disconnected()` events. All three take `&mut self`.
- **`TcpServer`**: Accepts incoming TCP connections. Requires `ApplicationStates` and `Logger` from `rust_extensions`.
- **`TcpClient`**: Connects to a remote server with auto-reconnect. Requires `TcpClientSocketSettings` to provide host/port and optional TLS.
- **`UnixSocketServer`** (Unix only): Same as `TcpServer` but listens on a Unix Domain Socket.
- **`TcpWriteBuffer`**: Trait for writing bytes. Provides helpers: `write_byte()`, `write_i32()`, `write_u64()`, `write_byte_array()`, `write_pascal_string()`, etc.
- **`SocketReader`**: Symmetric read-side trait with matching `read_byte()`, `read_i32()`, `read_byte_array()`, `read_buf()`, etc.
- **`ConnectionId`** (alias for `i32`): Globally unique identifier assigned per `TcpSocketConnection` from a process-wide counter, so IDs do not repeat across reconnects or across server/client instances inside the same process.

## Important: send helpers are `async fn` — you MUST `.await` them

In this version of the library `send`, `send_many`, `send_bytes`, and `send_ping` are all declared `async fn` and return `usize` (bytes queued; `0` if the connection is closed). They internally lock an async `Mutex` over the outgoing buffer, so calling them without `.await` produces a `Future` that is dropped immediately — **the bytes are never enqueued**. Always `.await`:

```rust
// CORRECT
connection.send(&contract).await;
connection.send_bytes(&payload).await;
connection.send_many(&batch).await;

// WRONG — Future is dropped, nothing is sent (compiler emits unused_must_use)
connection.send(&contract);
connection.send_bytes(&payload);
```

If you find existing code that calls these without `.await`, treat it as a bug.

## Minimal protocol example
```rust
use async_trait::async_trait;
use my_tcp_sockets::{
    socket_reader::ReadingTcpContractFail, socket_reader::SocketReader, TcpContract,
    TcpSerializerFactory, TcpSerializerState, TcpSocketSerializer, TcpWriteBuffer,
};

#[derive(Clone, Debug)]
struct Chat {
    text: String,
}

impl TcpContract for Chat {
    fn is_pong(&self) -> bool {
        self.text == "PONG"
    }
}

#[derive(Clone, Default)]
struct ChatState;
impl TcpSerializerState<Chat> for ChatState {
    fn is_tcp_contract_related_to_metadata(&self, _contract: &Chat) -> bool {
        false
    }
    fn apply_tcp_contract(&mut self, _contract: &Chat) {}
}

struct ChatSerializer;
#[async_trait]
impl TcpSocketSerializer<Chat, ChatState> for ChatSerializer {
    fn serialize(&self, out: &mut impl TcpWriteBuffer, contract: &Chat, _state: &ChatState) {
        // TcpWriteBuffer provides many helpers:
        // write_byte(), write_i16(), write_u16(), write_i32(), write_u32(),
        // write_i64(), write_u64(), write_bool(), write_byte_array(),
        // write_pascal_string(), serialize_list_of_arrays(), etc.
        out.write_byte_array(contract.text.as_bytes());
    }

    fn get_ping(&self) -> Chat {
        Chat { text: "PING".into() }
    }

    async fn deserialize<TR: SocketReader + Send + Sync + 'static>(
        &mut self,
        reader: &mut TR,
        _state: &ChatState,
    ) -> Result<Chat, ReadingTcpContractFail> {
        // SocketReader provides matching read methods:
        // read_byte(), read_i32(), read_u32(), read_i64(), read_u64(),
        // read_bool(), read_byte_array(), read_buf(), etc.
        let bytes = reader.read_byte_array().await?;
        let text = String::from_utf8(bytes).unwrap_or_default();
        Ok(Chat { text })
    }
}

struct ChatFactory;
#[async_trait]
impl TcpSerializerFactory<Chat, ChatSerializer, ChatState> for ChatFactory {
    async fn create_serializer(&self) -> ChatSerializer {
        ChatSerializer
    }
    async fn create_serializer_state(&self) -> ChatState {
        ChatState::default()
    }
}
```

**Serialization helpers**: `TcpWriteBuffer` and `SocketReader` provide symmetric read/write methods for common types. Use `write_byte_array()` / `read_byte_array()` for length-prefixed byte arrays, `write_pascal_string()` for length-prefixed strings (max 255 bytes), or implement custom framing.

## Handling socket events

`SocketEventCallback` hooks all receive `&mut self`, so your implementation can mutate internal state (counters, caches, per-connection maps) without extra synchronisation.

```rust
use std::sync::Arc;
use async_trait::async_trait;
use my_tcp_sockets::{SocketEventCallback, tcp_connection::TcpSocketConnection};

#[derive(Clone)]
struct Echo;

#[async_trait]
impl SocketEventCallback<Chat, ChatSerializer, ChatState> for Echo {
    async fn connected(
        &mut self,
        connection: Arc<TcpSocketConnection<Chat, ChatSerializer, ChatState>>,
    ) {
        // .await is required — see "send helpers are async fn" above.
        connection.send(&Chat { text: "hello".into() }).await;
    }

    async fn payload(
        &mut self,
        connection: &Arc<TcpSocketConnection<Chat, ChatSerializer, ChatState>>,
        contract: Chat,
    ) {
        // Echo back every message
        connection.send(&contract).await;
    }

    async fn disconnected(
        &mut self,
        _connection: Arc<TcpSocketConnection<Chat, ChatSerializer, ChatState>>,
    ) {
        // clean up resources if needed
    }
}
```

Callback bounds:
- `TcpServer` / `UnixSocketServer`: `SocketEventCallback + Send + Clone + 'static` — the server clones the callback per accepted connection.
- `TcpClient`: `SocketEventCallback + Send + 'static` — the client owns a single callback instance for the reconnect loop. `Sync` is **not** required in either case.

## TcpSocketConnection — full method surface

All of the following are available on `Arc<TcpSocketConnection<...>>` you receive in the callbacks. Methods marked `async` must be `.await`-ed.

| Method | Sig | Notes |
|--------|-----|-------|
| `as_i32(&self) -> i32` | sync | Numeric `ConnectionId`. |
| `is_finished(&self) -> bool` | sync | True after the read/write loops have exited. |
| `is_connected(&self) -> bool` | sync | False once the socket has dropped. **Always check before sending if you care about delivery**, since a closed connection silently drops outgoing messages. |
| `disconnect(&self) -> bool` | async | Closes the socket. Returns `true` if it actually transitioned from connected → disconnected. |
| `send(&self, contract: &TContract) -> usize` | **async** | Serializes through the configured `TcpSocketSerializer`, then enqueues. |
| `send_many(&self, contracts: &[TContract]) -> usize` | **async** | Batched variant of `send`. |
| `send_bytes(&self, payload: &[u8]) -> usize` | **async** | Bypasses the serializer; enqueues a pre-encoded frame. |
| `send_ping(&self) -> usize` | async | Built from the serializer's `get_ping()`. The keep-alive loop calls this for you; you rarely call it manually. |
| `set_connection_name(&self, name: String)` | async | Updates the human-readable name used in logs. |
| `update_incoming_packet_to_state(&self, contract: &TContract)` | async | Manually feed an inbound contract into the per-connection `TcpSerializerState`. The library does this automatically when `is_tcp_contract_related_to_metadata` returns `true`. |
| `get_log_context(&self)` | async | Returns the connection's logging key/value map. |
| `update_read_thread_status(&self, status)` / `get_read_thread_status(&self)` / `get_write_thread_status(&self)` | sync | Used by the dead-connection detector. |
| `statistics(&self) -> &ConnectionStatistics` | sync | See "Connection statistics" below. |
| `is_dead(&self, now: DateTimeAsMicroseconds) -> bool` | sync | True if the dead-connection detector should kill this socket. |

**Note:** there is **no** `send_and_await_next_payload` method in this version. If you need request/response correlation (e.g. correlate a TWIME `NewOrderSingle` with the incoming `ExecutionReport` that has the same `cl_ord_id`), you have to build it yourself: keep a map of `cl_ord_id -> TaskCompletion`, call `connection.send_bytes(&bytes).await` to fire, and have your `payload()` callback complete the awaiter when a matching contract arrives.

## Running a server
```rust
use std::{net::SocketAddr, sync::Arc};
use my_tcp_sockets::TcpServer;

let server = TcpServer::new("chat-server".to_string(), "0.0.0.0:7000".parse::<SocketAddr>()?);

// Provide your own ApplicationStates + Logger implementations from rust_extensions
let app_states = /* Arc<dyn ApplicationStates> */ todo!();
let logger = /* Arc<dyn Logger> */ todo!();

server
    .start(
        Arc::new(ChatFactory),
        Echo, // callback passed by value; must be Clone
        app_states,
        logger,
    )
    .await;
```

## Running a client
```rust
use std::{sync::Arc, time::Duration};
use async_trait::async_trait;
use my_tcp_sockets::{TcpClient, TcpClientSocketSettings, TlsSettings};

struct StaticSettings {
    addr: String,
    tls: Option<TlsSettings>,
}

#[async_trait]
impl TcpClientSocketSettings for StaticSettings {
    async fn get_host_port(&self) -> Option<String> {
        Some(self.addr.clone())
    }
    async fn get_tls_settings(&self) -> Option<TlsSettings> {
        self.tls.clone()
    }
}

let client = TcpClient::new(
    "chat-client".to_string(),
    Arc::new(StaticSettings {
        addr: "127.0.0.1:7000".into(),
        tls: None, // or Some(TlsSettings { server_name: "example.com".into() })
    }),
)
.set_seconds_to_ping(5)                            // Send ping every 5 seconds
.set_disconnect_timeout(Duration::from_secs(15))   // Disconnect if no data for 15s
.set_reconnect_timeout(Duration::from_secs(3));    // Wait 3s between reconnect attempts

let logger = /* Arc<dyn Logger> */ todo!();
client
    .start::<Chat, ChatSerializer, ChatState, ChatFactory, Echo>(
        Arc::new(ChatFactory),
        Echo, // callback passed by value; single instance per client
        logger,
    )
    .await;

// Later: drop the active socket — the reconnect loop will re-establish it.
client.try_disconnect_current_connection().await;

// Shutdown client completely (no further reconnect attempts).
client.stop().await;
```

**Note**: If `get_host_port()` returns `None`, the client will skip that connection attempt and retry after `reconnect_timeout`. This is useful for dynamic configuration where the endpoint might not be available yet.

### TLS on the client (feature `with-tls`)
When `get_tls_settings()` returns `Some(TlsSettings { server_name })`, the client performs a rustls TLS handshake over the freshly opened TCP stream (using the bundled root certificate store from `my_tls::ROOT_CERT_STORE`) and wraps the read/write halves as `MaybeTls{Read,Write}Stream::Tls`. Handshake failures are logged and the reconnect loop retries after `reconnect_timeout`. Returning `None` keeps the connection plain TCP. TLS requires the `with-tls` feature; without it `get_tls_settings()` is ignored at build time.

## Unix Domain Socket server (Unix only)
```rust
use std::sync::Arc;
use my_tcp_sockets::UnixSocketServer;

let unix_server = UnixSocketServer::new(
    "my-unix-server",
    "/tmp/my-socket.sock"  // or "~/.my-socket" for home directory
);

let app_states = /* Arc<dyn ApplicationStates> */ todo!();
let logger = /* Arc<dyn Logger> */ todo!();

unix_server
    .start(
        Arc::new(ChatFactory),
        Echo, // callback passed by value; must be Clone
        app_states,
        logger,
    )
    .await;
```

Clients can connect to Unix sockets by using a path starting with `/` or `~` in `get_host_port()`.

## Advanced usage

### Sending multiple messages
```rust
// Send multiple contracts in one batch (all .await-ed)
let messages = vec![
    Chat { text: "msg1".into() },
    Chat { text: "msg2".into() },
    Chat { text: "msg3".into() },
];
let sent_count = connection.send_many(&messages).await;

// Send raw bytes (bypasses serializer)
let raw_data = b"raw bytes";
connection.send_bytes(raw_data).await;
```

### Building request/response yourself
There is no built-in `send_and_await_next_payload`. The pattern most projects use:

```rust
// Pseudocode — adapt to your contract.
struct ActiveRequests {
    map: ahash::AHashMap<u64, TaskCompletion<MyContract, String>>,
}

// 1. Register the awaiter, keyed by your correlation id (cl_ord_id, request_id, …).
let mut completion = TaskCompletion::<MyContract, String>::new();
let awaiter = completion.get_awaiter();
active_requests.lock().await.map.insert(req_id, completion);

// 2. Fire the request — note the .await.
connection.send_bytes(&bytes).await;

// 3. Wait with a timeout you control.
let result = tokio::time::timeout(Duration::from_secs(3), awaiter.get_result()).await;

// 4. In SocketEventCallback::payload, look up the completion by req_id and call set_ok(...).
//    In SocketEventCallback::disconnected, drain the map and set_panic on each so awaiters wake up.
```

### Connection statistics
```rust
let stats = connection.statistics();

// Check connection timing
let connected_at = stats.connected;
let last_send = stats.last_send_moment.as_date_time();
let last_receive = stats.last_receive_moment.as_date_time();

// Monitor throughput
let total_received = stats.total_received.load(std::sync::atomic::Ordering::Relaxed);
let total_sent = stats.total_sent.load(std::sync::atomic::Ordering::Relaxed);
let received_per_sec = stats.received_per_sec.get();
let sent_per_sec = stats.sent_per_sec.get();

// Ping/pong round-trip time
if let Some(rtt) = stats.get_ping_pong_duration() {
    println!("Round-trip time: {:?}", rtt);
}
```

### State management with incoming packets
When a contract affects connection state (e.g., authentication, session setup), the library automatically calls `apply_tcp_contract` on your state if `is_tcp_contract_related_to_metadata` returns `true` — and it does so **before** dispatching the contract to `payload()`. You can also update state manually:

```rust
#[async_trait]
impl SocketEventCallback<Chat, ChatSerializer, ChatState> for Echo {
    async fn payload(
        &mut self,
        connection: &Arc<TcpSocketConnection<Chat, ChatSerializer, ChatState>>,
        contract: Chat,
    ) {
        // Manually feed a contract into per-connection state (rarely needed).
        connection.update_incoming_packet_to_state(&contract).await;

        connection.send(&contract).await;
    }
}
```

### Thread statistics monitoring
```rust
let stats = &server.threads_statistics; // or client.inner.threads_statistics

let read_threads = stats.read_threads.get();
let write_threads = stats.write_threads.get();
let ping_threads = stats.ping_threads.get();
let active_connections = stats.connections_objects.get();
```

## Error handling

### Deserialization errors
If `deserialize()` returns `ReadingTcpContractFail`, the connection is automatically closed and the `disconnected()` callback is invoked. Common causes:
- Socket closed unexpectedly
- Invalid protocol data
- Timeout during read (configurable per connection)

### Send errors
- `send()`, `send_many()`, `send_bytes()` return the number of bytes queued (`0` if the connection is closed at the time of the call).
- These methods do not return `Result` — a closed connection silently drops the message. Check `connection.is_connected()` before sending if delivery matters.
- The send pipeline has an internal write timeout; the connection is dropped (and the reconnect loop kicks in for `TcpClient`) if the write side stalls.

### Connection lifecycle
- **Server**: Connections are accepted in a background task. If `ApplicationStates::is_shutting_down()` returns `true`, new connections are rejected.
- **Client**: If `get_host_port()` returns `None`, the client skips that attempt and retries after `reconnect_timeout`. Useful for dynamic configuration.
- Dead connections are detected via ping/pong timeout (`disconnect_timeout`). The connection is closed and `disconnected()` is called.

## Tips
- Use `client.try_disconnect_current_connection().await` to drop the active socket; the loop will reconnect.
- On Unix you can also connect via Unix Domain Sockets; enable `with-tls` for TLS.
- Inspect `threads_statistics` to monitor read/write threads and active connections.
- Connection statistics are updated automatically; access them via `connection.statistics()`.
- Ping/pong round-trip time is measured automatically when `is_pong()` returns `true` for a received contract.
- If `TcpSerializerState::is_tcp_contract_related_to_metadata` returns `true`, the contract is applied to state before calling `payload()`.
- Always check `connection.is_connected()` before sending if you need to handle disconnections gracefully.
- Use `send_many().await` for batch operations; it's more efficient than multiple `send().await` calls.

## Real-world usage: My Service Bus SDK
`my-service-bus-sdk` builds on `my-tcp-sockets` to keep a long-lived TCP channel to the bus, serialize custom contracts, and auto-reconnect with backoff. Key patterns you can mirror ([repo](https://github.com/MyJetTools/my-service-bus-sdk)):
- Wrap `TcpClient` behind a settings trait to supply `host:port` (optionally TLS) from dynamic config.
- Use a serializer/state pair to map your bus contract to bytes and detect `PONG` replies for keep-alive.
- Run a background client loop; on disconnect, retries continue publishing when connectivity returns.
- Implement callbacks to process inbound payloads (e.g., dispatch bus messages to subscribers) and to clean up on disconnect.
- Create higher-level helpers (publishers/subscribers) atop `send`, `send_many`, and ping/pong so application code never touches sockets directly.

## Real-world usage: My NoSQL SDK
`my-no-sql-sdk` also layers on `my-tcp-sockets` to stream data partitions over TCP with reconnect and contract-level serialization ([repo](https://github.com/MyJetTools/my-no-sql-sdk)):
- Wraps `TcpClient` behind a settings provider that yields `host:port` for the NoSQL gateway, keeping connection details outside business logic.
- Uses custom serializer/state to encode partitions, rows, and sync markers; `TcpContract::is_pong` participates in the same ping/pong keep-alive.
- Runs a background loop to resubscribe partitions after reconnects so consumers keep receiving updates without manual intervention.
- Builds higher-level primitives (writers/readers) atop socket callbacks, so application code interacts with domain objects instead of raw TCP frames.
