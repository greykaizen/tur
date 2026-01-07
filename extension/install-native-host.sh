#!/bin/bash
# Install tur native messaging host for Chrome/Chromium
#
# This script:
# 1. Copies tur-host binary to /usr/local/bin
# 2. Creates native messaging host manifest
# 3. Registers manifest with Chrome/Chromium
#
# Usage: sudo ./install-native-host.sh [EXTENSION_ID]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
EXTENSION_ID="${1:-EXTENSION_ID_HERE}"
HOST_NAME="com.greykaizen.tur"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

echo "Installing tur native messaging host..."

# Check for tur-host binary
if [ ! -f "${SCRIPT_DIR}/../src-tauri/target/debug/tur-host" ] && [ ! -f "${SCRIPT_DIR}/../src-tauri/target/release/tur-host" ]; then
    echo -e "${RED}Error: tur-host binary not found. Run 'cargo build --bin tur-host' first.${NC}"
    exit 1
fi

# Use release binary if available, otherwise debug
if [ -f "${SCRIPT_DIR}/../src-tauri/target/release/tur-host" ]; then
    HOST_BINARY="${SCRIPT_DIR}/../src-tauri/target/release/tur-host"
else
    HOST_BINARY="${SCRIPT_DIR}/../src-tauri/target/debug/tur-host"
fi

# Copy binary
echo "Copying tur-host to /usr/local/bin..."
sudo cp "$HOST_BINARY" /usr/local/bin/tur-host
sudo chmod +x /usr/local/bin/tur-host

# Create manifest
MANIFEST_DIR="$HOME/.config/chromium/NativeMessagingHosts"
mkdir -p "$MANIFEST_DIR"

cat > "$MANIFEST_DIR/${HOST_NAME}.json" << EOF
{
    "name": "${HOST_NAME}",
    "description": "tur download manager native messaging host",
    "path": "/usr/local/bin/tur-host",
    "type": "stdio",
    "allowed_origins": [
        "chrome-extension://${EXTENSION_ID}/"
    ]
}
EOF

echo -e "${GREEN}✓ Native messaging host installed successfully!${NC}"
echo ""
echo "Manifest installed to: $MANIFEST_DIR/${HOST_NAME}.json"
echo ""
echo "Next steps:"
echo "1. Load the extension in Chrome and note its ID"
echo "2. Update the manifest with the correct extension ID:"
echo "   Run: ./install-native-host.sh YOUR_EXTENSION_ID"
