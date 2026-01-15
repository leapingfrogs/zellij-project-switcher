#!/bin/bash
# Source this file to set up WASI SDK environment variables
# Usage: source ./scripts/env.sh

# Find project root (works whether sourced or executed)
if [ -n "${BASH_SOURCE[0]}" ]; then
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
else
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
fi
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Also check current directory as fallback
if [ ! -d "$PROJECT_DIR/.wasi-sdk" ] && [ -d ".wasi-sdk" ]; then
    PROJECT_DIR="$(pwd)"
fi

WASI_SDK_DIR="$PROJECT_DIR/.wasi-sdk"

if [ ! -d "$WASI_SDK_DIR" ]; then
    echo "WASI SDK not found at $WASI_SDK_DIR"
    echo "Run ./scripts/setup-wasi-sdk.sh first."
    return 1 2>/dev/null || exit 1
fi

export WASI_SDK_PATH="$WASI_SDK_DIR"
export CC="$WASI_SDK_DIR/bin/clang"

echo "WASI SDK environment configured:"
echo "  WASI_SDK_PATH=$WASI_SDK_PATH"
echo "  CC=$CC"
