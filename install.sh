#!/usr/bin/env bash
set -e

echo -e "\033[36mEider CLI Installer\033[0m"
echo "───────────────────"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

if [ "$ARCH" = "x86_64" ]; then
    ARCH="amd64"
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    ARCH="arm64"
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

DUCKDB_PLATFORM=""
if [ "$OS" = "darwin" ] && [ "$ARCH" = "arm64" ]; then
    DUCKDB_PLATFORM="osx_arm64"
elif [ "$OS" = "darwin" ] && [ "$ARCH" = "amd64" ]; then
    DUCKDB_PLATFORM="osx_amd64"
elif [ "$OS" = "linux" ] && [ "$ARCH" = "amd64" ]; then
    DUCKDB_PLATFORM="linux_amd64"
elif [ "$OS" = "linux" ] && [ "$ARCH" = "arm64" ]; then
    DUCKDB_PLATFORM="linux_arm64"
else
    echo "Unsupported OS/Arch combination for extension: $OS $ARCH"
    exit 1
fi

BIN_DIR="$HOME/.local/bin"
EXT_DIR="$HOME/.duckdb/extensions/v1.1.0/$DUCKDB_PLATFORM"

mkdir -p "$BIN_DIR"
mkdir -p "$EXT_DIR"

echo "Fetching latest release..."
# For now, we simulate fetching the latest release from the repository.
# You would curl the GitHub API here. Since we are in development, we'll just print instructions.
echo -e "\033[35mTarget Bin:\033[0m $BIN_DIR/eider"
echo -e "\033[35mTarget Ext:\033[0m $EXT_DIR/eider_extension.duckdb_extension"

echo -e "\n\033[32m✔\033[0m Installation simulated successfully."
echo "Please add $BIN_DIR to your PATH if it is not already."
