#!/bin/bash
# Setup WASI SDK for local development
# This script installs WASI SDK to enable WASM testing with C dependencies

set -e

WASI_SDK_VERSION="29"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
INSTALL_DIR="$PROJECT_DIR/.wasi-sdk"

# Detect platform
case "$(uname -s)-$(uname -m)" in
    Linux-x86_64)
        PLATFORM="x86_64-linux"
        ;;
    Linux-aarch64)
        PLATFORM="arm64-linux"
        ;;
    Darwin-arm64)
        PLATFORM="arm64-macos"
        ;;
    Darwin-x86_64)
        PLATFORM="x86_64-macos"
        ;;
    *)
        echo "Unsupported platform: $(uname -s)-$(uname -m)"
        exit 1
        ;;
esac

ARCHIVE="wasi-sdk-${WASI_SDK_VERSION}.0-${PLATFORM}.tar.gz"
URL="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-${WASI_SDK_VERSION}/${ARCHIVE}"

echo "==> Detected platform: ${PLATFORM}"

# Check if already installed
if [ -d "$INSTALL_DIR" ] && [ -x "$INSTALL_DIR/bin/clang" ]; then
    echo "==> WASI SDK already installed at $INSTALL_DIR"
    echo ""
    echo "To use it, run:"
    echo "  source ./scripts/env.sh"
    exit 0
fi

echo "==> Downloading WASI SDK ${WASI_SDK_VERSION} for ${PLATFORM}..."
curl -fsSL -o "$ARCHIVE" "$URL"

echo "==> Extracting..."
tar xzf "$ARCHIVE"

echo "==> Installing to $INSTALL_DIR..."
rm -rf "$INSTALL_DIR"
mv "wasi-sdk-${WASI_SDK_VERSION}.0-${PLATFORM}" "$INSTALL_DIR"

echo "==> Cleaning up..."
rm -f "$ARCHIVE"

echo ""
echo "==> WASI SDK installed successfully!"
echo ""
echo "To use it, run:"
echo "  source ./scripts/env.sh"
echo ""
echo "Then run: cargo test"
