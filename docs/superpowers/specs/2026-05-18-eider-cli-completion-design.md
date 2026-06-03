# Eider CLI Shell Auto-Completion Design

**Date:** 2026-05-18

## 1. Context & Purpose
To maximize user-friendliness, the `eider` CLI should support shell auto-completion. This allows users to press `<TAB>` in their terminal to automatically complete command names (e.g., `info`, `extract`, `shell`) and their respective arguments without having to constantly reference the `--help` menu.

The purpose of this sub-project is to implement a runtime `completions` command using the `clap_complete` crate. This allows users to generate the completion scripts dynamically for their preferred shell and pipe the output directly into their shell configuration files.

## 2. Core Architecture & Dependencies

We will use the **`clap_complete`** crate, which integrates seamlessly with our existing `clap::Parser` derivation to generate 100% accurate completion scripts based on our Rust struct definitions.

The new dependency in `cli/Cargo.toml` will be:
- `clap_complete = "4.5"`

## 3. The `completions` Command

We will add a new subcommand to the `Commands` enum in `main.rs`:

```rust
    /// Generate shell completion scripts
    Completions {
        /// The shell to generate completions for (bash, elvish, fish, powershell, zsh)
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
```

## 4. Execution Logic

When the `Completions` command is invoked, the execution flow will be:
1. Extract the `clap::Command` representation of our `Cli` struct using `<Cli as clap::CommandFactory>::command()`.
2. Invoke `clap_complete::generate()`, passing the requested shell enum, the command representation, the binary name (`"eider"`), and a mutable reference to standard output (`&mut std::io::stdout()`).
3. Exit cleanly.

### Example User Workflow
A user running Zsh would execute:
`eider completions zsh > ~/.zfunc/_eider`

## 5. Agent & Output Format Constraints
The global `--output` flag (which defaults to `table` but can be `json`) is technically available on all commands. However, the `completions` command's sole purpose is to output raw shell script text.

Therefore, if the `completions` command is invoked, the CLI will bypass any JSON wrapping logic and directly stream the raw shell script to standard output, ensuring the piped output remains valid shell code regardless of the global output format setting.

## 6. Testing Strategy
- A new integration test using `assert_cmd` will invoke `eider completions bash` and assert that the output string contains standard bash completion syntax (e.g., `_eider() {`).
