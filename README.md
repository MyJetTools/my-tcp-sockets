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
my-tcp-sockets = { git = "https://github.com/MyJetTools/MyTcpSockets.git", tag = "0.1.12" }
# Enable TLS if needed
# my-tcp-sockets = { git = "https://github.com/MyJetTools/MyTcpSockets.git", tag = "0.1.12", features = ["with-tls"] }
```

## Core concepts
- **`TcpContract`**: Your message type; implement `is_pong()` so the keep-alive can spot pong replies.
- **`TcpSocketSerializer`**: Serializes contracts to bytes and deserializes from socket reader. Provides `get_ping()` for keep-alive.
- **`TcpSerializerState`**: Per-connection state that can be updated by incoming contracts. Use `is_tcp_contract_related_to_metadata()` to filter which contracts update state.
- **`TcpSerializerFactory`**: Creates serializer and state instances for each new connection.
- **`SocketEventCallback`**: Async hooks for `connected()`, `payload()`, `disconnected()` events.
- **`TcpServer`**: Accepts incoming TCP connections. Requires `ApplicationStates` and `Logger` from `rust_extensions`.
- **`TcpClient`**: Connects to a remote server with auto-reconnect. Requires `TcpClientSocketSettings` to provide host/port and optional TLS.
- **`TcpWriteBuffer`**: Trait for writing bytes. Provides helpers: `write_byte()`, `write_i32()`, `write_u64()`, `write_byte_array()`, `write_pascal_string()`, etc.

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
        Chat {
            text: "PING".into(),
        }
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
```rust
use std::sync::Arc;
use async_trait::async_trait;
use my_tcp_sockets::{SocketEventCallback, tcp_connection::TcpSocketConnection};

struct Echo;

#[async_trait]
impl SocketEventCallback<Chat, ChatSerializer, ChatState> for Echo {
    async fn connected(
        &self,
        connection: Arc<TcpSocketConnection<Chat, ChatSerializer, ChatState>>,
    ) {
        connection.send(&Chat { text: "hello".into() }).await;
    }

    async fn payload(
        &self,
        connection: &Arc<TcpSocketConnection<Chat, ChatSerializer, ChatState>>,
        contract: Chat,
    ) {
        // Echo back every message
        connection.send(&contract).await;
    }

    async fn disconnected(
        &self,
        _connection: Arc<TcpSocketConnection<Chat, ChatSerializer, ChatState>>,
    ) {
        // clean up resources if needed
    }
}
```

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
        Arc::new(Echo),
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
.set_seconds_to_ping(5)           // Send ping every 5 seconds
.set_disconnect_timeout(Duration::from_secs(15))  // Disconnect if no data for 15s
.set_reconnect_timeout(Duration::from_secs(3));  // Wait 3s between reconnect attempts

let logger = /* Arc<dyn Logger> */ todo!();
client
    .start::<Chat, ChatSerializer, ChatState, ChatFactory, Echo>(
        Arc::new(ChatFactory),
        Arc::new(Echo),
        logger,
    )
    .await;

// Later: manually disconnect (will auto-reconnect)
client.try_disconnect_current_connection().await;

// Shutdown client completely
client.stop().await;
```

**Note**: If `get_host_port()` returns `None`, the client will skip that connection attempt and retry after `reconnect_timeout`. This is useful for dynamic configuration where the endpoint might not be available yet.

## Unix Domain Socket Server (Unix only)
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
        Arc::new(Echo),
        app_states,
        logger,
    )
    .await;
```

Clients can connect to Unix sockets by using a path starting with `/` or `~` in `get_host_port()`.

## Advanced usage

### Sending multiple messages
```rust
// Send multiple contracts in one batch
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
When a contract affects connection state (e.g., authentication, session setup), the library automatically calls `apply_tcp_contract` on your state if `is_tcp_contract_related_to_metadata` returns true. You can also manually update state:

```rust
#[async_trait]
impl SocketEventCallback<Chat, ChatSerializer, ChatState> for Echo {
    async fn payload(
        &self,
        connection: &Arc<TcpSocketConnection<Chat, ChatSerializer, ChatState>>,
        contract: Chat,
    ) {
        // Manually update state if needed (usually automatic)
        connection.update_incoming_packet_to_state(&contract).await;
        
        // Process the message
        connection.send(&contract).await;
    }
}
```

**Note**: The library automatically applies state updates for contracts where `is_tcp_contract_related_to_metadata` returns true before calling `payload()`. Use `update_incoming_packet_to_state` only if you need manual control.

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
If `deserialize()` returns `ReadingTcpContractFail`, the connection is automatically closed and `disconnected()` callback is invoked. Common causes:
- Socket closed unexpectedly
- Invalid protocol data
- Timeout during read (configurable per connection)

### Send errors
- `send()`, `send_many()`, `send_bytes()` return the number of bytes queued (0 if connection is closed).
- If send buffer is full or connection is closed, messages are silently dropped. Check `connection.is_connected()` before sending.
- Send operations have a timeout (default 30s, configurable via `TcpServer`/`TcpClient`).

### Connection lifecycle
- **Server**: Connections are accepted in a background task. If `ApplicationStates::is_shutting_down()` returns true, new connections are rejected.
- **Client**: If `get_host_port()` returns `None`, the client skips that attempt and retries after `reconnect_timeout`. Useful for dynamic configuration.
- Dead connections are detected via ping/pong timeout (`disconnect_timeout`). The connection is closed and `disconnected()` is called.

## Tips
- Use `client.try_disconnect_current_connection().await` to drop the active socket; the loop will reconnect.
- On Unix you can also connect via Unix Domain Sockets; enable `with-tls` for TLS.
- Inspect `threads_statistics` to monitor read/write threads and active connections.
- Connection statistics are updated automatically; access them via `connection.statistics()`.
- Ping/pong round-trip time is measured automatically when `is_pong()` returns true for a received contract.
- If `TcpSerializerState::is_tcp_contract_related_to_metadata` returns true, the contract is applied to state before calling `payload()` callback.
- Always check `connection.is_connected()` before sending if you need to handle disconnections gracefully.
- Use `send_many()` for batch operations; it's more efficient than multiple `send()` calls.

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
