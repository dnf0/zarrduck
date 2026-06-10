use rmcp::model::CallToolRequestParams;
use rmcp::ServiceExt;
use serde_json::{Map, Value};
use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::process::Command;

/// The tools the adapter must expose.
const EXPECTED_TOOLS: [&str; 8] = [
    "describe_dataset",
    "estimate_cost",
    "read_region",
    "zonal_stats",
    "list_tables",
    "describe_table",
    "run_sql",
    "extract_point_timeseries",
];

/// Skip-guard: returns false (and prints) when the eider extension is not built.
fn extension_available() -> bool {
    // Match the path resolution used by `eider_session`/the unit tests.
    std::env::set_var(
        "GEOZARR_ALLOW_PATH",
        format!("{}/..", env!("CARGO_MANIFEST_DIR")),
    );
    if std::env::var("EIDER_EXTENSION_PATH").is_err()
        && !std::path::Path::new("../target/debug/eider.duckdb_extension").exists()
    {
        eprintln!("skip: eider extension not built");
        return false;
    }
    true
}

fn zarr_uri() -> String {
    format!(
        "{}/../climate_data.zarr/air_temperature",
        env!("CARGO_MANIFEST_DIR")
    )
}

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

#[tokio::test]
async fn e2e_lifecycle() {
    if !extension_available() {
        return;
    }

    // Ensure the binary is built
    let bin_path = env!("CARGO_BIN_EXE_eider-mcp");

    let mut child = Command::new(bin_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()) // so we can see server logs in the test output
        .spawn()
        .expect("failed to spawn eider-mcp");

    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    let transport = ProcessTransport { stdin, stdout };

    // `serve` performs the initialize handshake for us.
    let client = ().serve(transport).await.expect("client initialize handshake");

    // tools/list: all tools present
    let listed = client.peer().list_tools(None).await.expect("tools/list");
    let names: Vec<String> = listed.tools.iter().map(|t| t.name.to_string()).collect();
    for expected in EXPECTED_TOOLS {
        assert!(
            names.contains(&expected.to_string()),
            "tools/list missing {expected}; got {names:?}"
        );
        let tool = listed
            .tools
            .iter()
            .find(|t| t.name == expected)
            .expect("tool present");
        let schema = tool.input_schema.as_ref();
        assert_eq!(
            schema.get("type").and_then(Value::as_str),
            Some("object"),
            "{expected} input schema should be an object schema"
        );
    }
    assert_eq!(
        listed.tools.len(),
        EXPECTED_TOOLS.len(),
        "exactly the curated tools should be exposed; got {names:?}"
    );

    // tools/call: describe_dataset against the committed sample.
    let mut args = Map::new();
    args.insert("uri".to_string(), Value::String(zarr_uri()));
    let result = client
        .peer()
        .call_tool(CallToolRequestParams::new("describe_dataset").with_arguments(args))
        .await
        .expect("tools/call describe_dataset");

    assert_ne!(result.is_error, Some(true), "describe_dataset errored");
    let structured = result
        .structured_content
        .expect("describe_dataset returns structured content");
    let text = structured.to_string();
    assert!(
        text.contains("EPSG:4326") && text.contains("Float32"),
        "describe_dataset payload missing CRS/dtype: {text}"
    );

    client.cancel().await.expect("client shutdown");

    // Once the client connection drops and stdin is closed, the server should gracefully exit.
    let status = child.wait().await.expect("wait for child");
    assert!(status.success(), "server did not exit successfully");
}
