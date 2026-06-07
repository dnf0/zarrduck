---
sidebar_position: 10
---

# eider completions

Generate a shell completion script and print it to stdout.

## Synopsis

```
eider completions <shell>
```

## Arguments

- `shell` — one of `bash`, `zsh`, `fish`, `powershell`, `elvish`.

## Example

```bash
# bash: load completions for the current session
source <(eider completions bash)

# or install persistently (bash)
eider completions bash > ~/.local/share/bash-completion/completions/eider
```
