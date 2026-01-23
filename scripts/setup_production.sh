#!/bin/bash
#
# Setup Osvauld Production Node
#
# This script:
# 1. Builds release binaries
# 2. Installs systemd service
# 3. Configures system to prevent sleep
# 4. Starts the service
#
# Usage:
#   ./scripts/setup_production.sh
#

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SERVICE_FILE="$SCRIPT_DIR/osvauld-production.service"

echo "========================================"
echo "  Osvauld Production Setup"
echo "========================================"
echo ""
echo "Project: $PROJECT_DIR"
echo ""

# Step 1: Build release binaries
echo "[1/5] Building release binaries..."
cd "$PROJECT_DIR"
cargo build --release -p slint_shell -p kunki
echo "  Done!"

# Update service file to use release binaries
echo "[2/5] Updating service for release binaries..."
# The TmuxManager uses target/debug by default, but we can symlink
ln -sf "$PROJECT_DIR/target/release/slint_shell" "$PROJECT_DIR/target/debug/slint_shell" 2>/dev/null || true
ln -sf "$PROJECT_DIR/target/release/kunki" "$PROJECT_DIR/target/debug/kunki" 2>/dev/null || true
echo "  Linked release binaries to debug path"

# Step 3: Install systemd service
echo "[3/5] Installing systemd service..."
sudo cp "$SERVICE_FILE" /etc/systemd/system/
sudo systemctl daemon-reload
echo "  Installed: /etc/systemd/system/osvauld-production.service"

# Step 4: Prevent laptop sleep
echo "[4/5] Configuring sleep prevention..."

# Method 1: systemd inhibit via logind.conf
LOGIND_CONF="/etc/systemd/logind.conf.d/no-sleep.conf"
sudo mkdir -p /etc/systemd/logind.conf.d/
sudo tee "$LOGIND_CONF" > /dev/null << 'EOF'
[Login]
# Prevent sleep when lid is closed (for laptop servers)
HandleLidSwitch=ignore
HandleLidSwitchExternalPower=ignore
HandleLidSwitchDocked=ignore

# Prevent idle sleep
IdleAction=ignore
EOF
echo "  Created: $LOGIND_CONF"

# Method 2: Mask sleep targets
sudo systemctl mask sleep.target suspend.target hibernate.target hybrid-sleep.target 2>/dev/null || true
echo "  Masked sleep targets"

# Reload logind
sudo systemctl restart systemd-logind 2>/dev/null || true
echo "  Reloaded logind"

# Step 5: Create data directory
echo "[5/5] Creating data directory..."
mkdir -p ~/.local/share/osvauld-production
echo "  Created: ~/.local/share/osvauld-production"

echo ""
echo "========================================"
echo "  Setup Complete!"
echo "========================================"
echo ""
echo "Commands:"
echo "  Start:   sudo systemctl start osvauld-production"
echo "  Stop:    sudo systemctl stop osvauld-production"
echo "  Status:  sudo systemctl status osvauld-production"
echo "  Logs:    journalctl -u osvauld-production -f"
echo "  Enable:  sudo systemctl enable osvauld-production  # auto-start on boot"
echo ""
echo "Tmux session: tmux attach -t osvauld_production"
echo ""
echo "To start now:"
echo "  sudo systemctl enable --now osvauld-production"
echo ""
