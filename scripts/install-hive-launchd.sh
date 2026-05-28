#!/usr/bin/env bash
# install-hive-launchd.sh — install the Hive's launchd jobs on macOS
#
# Usage:
#   ./scripts/install-hive-launchd.sh install
#   ./scripts/install-hive-launchd.sh uninstall
#   ./scripts/install-hive-launchd.sh status
#
# This script:
#   - Verifies you are on macOS
#   - Replaces the hardcoded paths in the .plist templates with the actual repo root
#   - Copies them to ~/Library/LaunchAgents/
#   - Loads them via launchctl
#
# Idempotent. Safe to run multiple times.

set -euo pipefail

ACTION="${1:-status}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LAUNCH_DIR="$HOME/Library/LaunchAgents"
TEMPLATE_DIR="$REPO_ROOT/.hive/launchd"

PLISTS=(
    "com.mycelium.hive.daily"
    "com.mycelium.hive.hourly"
    "com.mycelium.hive.nightly"
)

if [ "$(uname)" != "Darwin" ]; then
    echo "This installer is macOS-only. For Linux, write systemd units pointing at the same scripts/hive-run.sh."
    exit 1
fi

mkdir -p "$LAUNCH_DIR"

install_one() {
    local name="$1"
    local src="$TEMPLATE_DIR/${name}.plist"
    local dst="$LAUNCH_DIR/${name}.plist"

    if [ ! -f "$src" ]; then
        echo "Missing template: $src"
        return 1
    fi

    # Rewrite the hardcoded /Users/aisheng.yu/wiki/mycelium prefix to the actual repo root
    sed "s|/Users/aisheng.yu/wiki/mycelium|$REPO_ROOT|g" "$src" > "$dst"

    # Unload first if loaded (idempotent)
    launchctl unload "$dst" 2>/dev/null || true
    launchctl load "$dst"

    echo "  ✓ $name installed and loaded"
}

uninstall_one() {
    local name="$1"
    local dst="$LAUNCH_DIR/${name}.plist"

    if [ -f "$dst" ]; then
        launchctl unload "$dst" 2>/dev/null || true
        rm -f "$dst"
        echo "  ✓ $name uninstalled"
    else
        echo "  - $name not installed"
    fi
}

status_one() {
    local name="$1"
    local dst="$LAUNCH_DIR/${name}.plist"

    if [ -f "$dst" ]; then
        if launchctl list | grep -q "$name"; then
            echo "  ✓ $name : installed and loaded"
        else
            echo "  ⚠ $name : installed but not loaded"
        fi
    else
        echo "  ✗ $name : not installed"
    fi
}

case "$ACTION" in
    install)
        echo "Installing Hive launchd jobs from $TEMPLATE_DIR → $LAUNCH_DIR"
        mkdir -p "$REPO_ROOT/.hive/run" "$REPO_ROOT/.hive/audit"
        chmod +x "$REPO_ROOT/scripts/hive-run.sh"
        for p in "${PLISTS[@]}"; do install_one "$p"; done
        echo
        echo "Done. The Hive will start running on its schedules."
        echo "Logs: $REPO_ROOT/.hive/run/"
        echo "Audit: $REPO_ROOT/.hive/audit/"
        echo "Kill switch: close GitHub issue #1 to halt within 60 seconds."
        ;;
    uninstall)
        echo "Uninstalling Hive launchd jobs from $LAUNCH_DIR"
        for p in "${PLISTS[@]}"; do uninstall_one "$p"; done
        ;;
    status)
        echo "Hive launchd jobs status:"
        for p in "${PLISTS[@]}"; do status_one "$p"; done
        ;;
    *)
        echo "Usage: $0 {install|uninstall|status}"
        exit 1
        ;;
esac
