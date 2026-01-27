#!/bin/bash
set -e

# Configuration
TARGET="aarch64-unknown-linux-gnu"
REMOTE_HOST="root@xx.xx.xx.xx"
REMOTE_PATH="/root/decentpaste-relay"
BINARY_NAME="decentpaste-relay"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Building ${BINARY_NAME} for ${TARGET}...${NC}"

# Ensure the target is installed
rustup target add ${TARGET} 2>/dev/null || true

# Build release binary
cargo build --release --target ${TARGET}

BINARY_PATH="target/${TARGET}/release/${BINARY_NAME}"

if [ ! -f "${BINARY_PATH}" ]; then
    echo -e "${RED}Build failed: binary not found at ${BINARY_PATH}${NC}"
    exit 1
fi

# Get binary size
SIZE=$(ls -lh "${BINARY_PATH}" | awk '{print $5}')
echo -e "${GREEN}Build successful! Binary size: ${SIZE}${NC}"

echo -e "${YELLOW}Deploying to ${REMOTE_HOST}...${NC}"

# Transfer binary
scp "${BINARY_PATH}" "${REMOTE_HOST}:${REMOTE_PATH}"

# Make executable and optionally restart service
ssh "${REMOTE_HOST}" "chmod +x ${REMOTE_PATH} && echo 'Binary deployed successfully'"

echo -e "${GREEN}Deployment complete!${NC}"
echo -e "Run on server: ${REMOTE_PATH} --help"
