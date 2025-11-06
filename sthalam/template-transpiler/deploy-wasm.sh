#!/bin/bash

# WASM Build and Deploy Script
# Builds OCaml WASM modules and deploys them to the public directory

set -e  # Exit on error

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PUBLIC_DIR="$SCRIPT_DIR/../frontend/desktop/public"
BUILD_DIR="$SCRIPT_DIR/_build/default/eval-bin"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  WASM Build and Deploy${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Step 1: Clean previous build
echo -e "${YELLOW}[1/6]${NC} Cleaning previous build..."
cd "$SCRIPT_DIR"
opam exec -- dune clean

# Step 2: Build WASM modules
echo -e "${YELLOW}[2/6]${NC} Building WASM modules..."
opam exec -- dune build

# Step 3: Verify build output
echo -e "${YELLOW}[3/6]${NC} Verifying build output..."
if [ ! -f "$BUILD_DIR/cel_wasm.bc.wasm.js" ]; then
    echo -e "${RED}ERROR: cel_wasm.bc.wasm.js not found!${NC}"
    exit 1
fi

# Extract the WASM module hash
CEL_HASH=$(grep -o 'dune__exe__Cel_wasm-[a-f0-9]*' "$BUILD_DIR/cel_wasm.bc.wasm.js" | head -1 | cut -d'-' -f2)
echo -e "${GREEN}✓${NC} Built CEL WASM with hash: ${GREEN}${CEL_HASH}${NC}"

# Step 4: Make public files writable
echo -e "${YELLOW}[4/6]${NC} Preparing public directory..."
chmod -R u+w "$PUBLIC_DIR" 2>/dev/null || true

# Step 5: Remove old files
echo -e "${YELLOW}[5/6]${NC} Removing old WASM files..."
rm -rf "$PUBLIC_DIR/cel_wasm.bc.wasm.assets"
rm -f "$PUBLIC_DIR/cel_wasm.bc.wasm.js"
rm -f "$PUBLIC_DIR/cel_eval.js"

# Step 6: Copy new files
echo -e "${YELLOW}[6/6]${NC} Deploying new WASM files..."

# Copy WASM assets directory
cp -r "$BUILD_DIR/cel_wasm.bc.wasm.assets" "$PUBLIC_DIR/"
echo -e "${GREEN}✓${NC} Copied cel_wasm.bc.wasm.assets/"

# Copy JavaScript loader file (as both names)
cp "$BUILD_DIR/cel_wasm.bc.wasm.js" "$PUBLIC_DIR/cel_wasm.bc.wasm.js"
echo -e "${GREEN}✓${NC} Copied cel_wasm.bc.wasm.js"

cp "$BUILD_DIR/cel_wasm.bc.wasm.js" "$PUBLIC_DIR/cel_eval.js"
echo -e "${GREEN}✓${NC} Copied cel_eval.js"

# Fix permissions after copying (dune creates read-only files)
chmod -R u+w "$PUBLIC_DIR/cel_wasm.bc.wasm.assets" 2>/dev/null || true
chmod u+w "$PUBLIC_DIR/cel_wasm.bc.wasm.js" "$PUBLIC_DIR/cel_eval.js" 2>/dev/null || true

# Verify deployment
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Deployment Verification${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

DEPLOYED_HASH=$(grep -o 'dune__exe__Cel_wasm-[a-f0-9]*' "$PUBLIC_DIR/cel_eval.js" | head -1 | cut -d'-' -f2)
WASM_FILE="$PUBLIC_DIR/cel_wasm.bc.wasm.assets/dune__exe__Cel_wasm-${DEPLOYED_HASH}.wasm"

if [ -f "$WASM_FILE" ]; then
    echo -e "${GREEN}✓${NC} Deployed hash matches: ${GREEN}${DEPLOYED_HASH}${NC}"
    echo -e "${GREEN}✓${NC} WASM file exists: dune__exe__Cel_wasm-${DEPLOYED_HASH}.wasm"
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}  Deployment Successful!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
    echo "Please restart the Tauri app to load the new WASM modules."
else
    echo -e "${RED}✗${NC} WASM file not found: dune__exe__Cel_wasm-${DEPLOYED_HASH}.wasm"
    echo -e "${RED}Deployment verification failed!${NC}"
    exit 1
fi
