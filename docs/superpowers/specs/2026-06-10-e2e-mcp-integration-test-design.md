# End-to-End MCP Integration Testing Design

## Context
Currently, the `eider-mcp` protocol integration test (`mcp/tests/protocol.rs`) tests the MCP protocol logic by wiring the server directly to a client within the same process via an in-memory `tokio::io::duplex` transport.
While this guarantees the protocol structures are correct, it does not guarantee the actual compiled binary (`cargo run --bin eider-mcp`) handles `stdio` communication properly, boots successfully, and exits gracefully in a real-world setting.
We need an End-to-End (E2E) integration test that exercises the full lifecycle of the binary over the wire (via its OS `stdin` and `stdout` pipes).

## Approach
We will create a new integration test file `mcp/tests/e2e.rs`.

### 1. Spawning the Subprocess
The test will use `tokio::process::Command` to spawn the `eider-mcp` binary:
```rust
let mut child = Command::new(env!("CARGO_BIN_EXE_eider-mcp"))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit()) // so we can see server logs in the test output
    .spawn()
    .expect("failed to spawn eider-mcp");
```

### 2. Transport Bridging
The `rmcp` `serve(transport)` method expects a single object implementing both `AsyncRead` and `AsyncWrite`. Since `ChildStdout` implements `AsyncRead` and `ChildStdin` implements `AsyncWrite`, we will create a simple wrapper struct in the test to combine them:

```rust
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

struct ProcessTransport {
    stdin: tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
}

impl AsyncRead for ProcessTransport {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for ProcessTransport {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stdin).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_shutdown(cx)
    }
}
```

### 3. Lifecycle Execution
Using `ProcessTransport`, we will connect an `rmcp` client:
1. `().serve(transport).await` (Initializes the connection).
2. `client.peer().list_tools(None).await` (Verifies tools are accessible over the wire).
3. `client.peer().call_tool(...)` (Calls a simple, non-destructive tool like `describe_dataset` or `list_tables` to verify execution and response generation).
4. `client.cancel().await` (Sends shutdown/cancellation).
5. `child.wait().await` (Ensures the subprocess exits gracefully).

## Considerations & Trade-offs
- **Binary Dependency:** Using `env!("CARGO_BIN_EXE_eider-mcp")` requires `cargo test` to compile the binary before running the integration test. This is natively supported by Cargo in modern versions.
- **Flakiness:** Subprocess tests can occasionally be prone to deadlocks if stderr buffers fill up. Inheriting `stderr` directly to the test's `stderr` avoids this entirely.
- **Runtime:** This test will be slightly slower than `protocol.rs` due to the process startup overhead, but adds significant confidence in the shipped artifact.

## Acceptance Criteria
- `mcp/tests/e2e.rs` exists.
- `cargo test --test e2e` passes successfully.
- The subprocess is spawned and successfully killed/waited-for at the end of the test.
