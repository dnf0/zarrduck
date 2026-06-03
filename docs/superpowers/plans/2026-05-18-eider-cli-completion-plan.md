# Eider CLI Shell Auto-Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a `completions` subcommand to dynamically generate shell auto-completion scripts for various shells (bash, zsh, fish, etc.) directly to standard output.

**Architecture:** We will add the `clap_complete` crate to our dependencies. We will extend the `Commands` enum with a new `Completions` variant that takes a `Shell` enum as an argument. In the `run_cli` match block, we will invoke `clap_complete::generate` using our `Cli` struct's command factory and print the raw script to `std::io::stdout()`.

**Tech Stack:** Rust, `clap`, `clap_complete`

---

### Task 1: Add Dependency and Command Structure

**Files:**
- Modify: `cli/Cargo.toml`
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add `clap_complete` dependency**

Update `cli/Cargo.toml` to add `clap_complete`:
```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
clap_complete = "4.5"
duckdb = { version = "1.10502.0", features = ["bundled"] }
tokio = { version = "1.0", features = ["full"] }
opendal = { version = "0.48", features = ["services-s3", "services-http"] }
zarrs = { version = "0.16.4", features = ["opendal", "async"] }
serde_json = "1.0"
serde = { version = "1.0", features = ["derive"] }
color-eyre = "0.6"
indicatif = "0.17"
inquire = "0.7"
figment = { version = "0.10", features = ["toml", "env"] }
directories = "5.0"
```

- [ ] **Step 2: Add `Completions` to `Commands` enum**

In `cli/src/main.rs`, update the `Commands` enum to include the new subcommand. Also, import `CommandFactory` at the top of the file:

```rust
// Near top of cli/src/main.rs
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
// ...

#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Generate shell completion scripts
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}
```

- [ ] **Step 3: Run check to verify it compiles**

Run: `cargo check -p eider`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add cli/Cargo.toml cli/src/main.rs
git commit -m "feat: add clap_complete dependency and Completions subcommand"
```

---

### Task 2: Implement Generation Logic

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Implement `Completions` match arm**

In `cli/src/main.rs` inside the `run_cli` function's `match cli.command` block, add the implementation for `Completions`.

```rust
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        }
```

- [ ] **Step 2: Verify Compilation**

Run: `cargo check -p eider`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: implement shell auto-completion generation logic"
```

---

### Task 3: Test Auto-Completion Output

**Files:**
- Modify: `cli/tests/integration_test.rs`

- [ ] **Step 1: Write integration test for completion**

Add the following test to `cli/tests/integration_test.rs`:

```rust
#[test]
fn test_cli_completions_bash() {
    let mut cmd = Command::cargo_bin("eider").unwrap();
    cmd.arg("completions")
        .arg("bash")
        .assert()
        .success()
        .stdout(predicate::str::contains("_eider() {"));
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p eider --test integration_test test_cli_completions_bash`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add cli/tests/integration_test.rs
git commit -m "test: add integration test for bash auto-completion generation"
```
