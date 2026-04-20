#!/bin/bash
# Installation script for CPU Power Manager by JEGLY

set -e

if [ "$EUID" -ne 0 ]; then 
    echo "Please run as root (use sudo)"
    exit 1
fi

echo "=== CPU Power Manager Installation ==="
echo

# Check if binary exists
if [ ! -f "target/release/cpu-power-manager" ]; then
    echo "ERROR: Binary not found. Please run ./build.sh first"
    exit 1
fi

echo "Installing CPU Power Manager..."

# Install binary
install -D -m 0755 target/release/cpu-power-manager /usr/local/bin/cpu-power-manager
echo "✓ Installed binary to /usr/local/bin/cpu-power-manager"

# Install desktop file
install -D -m 0644 assets/cpu-power-manager.desktop /usr/share/applications/cpu-power-manager.desktop
echo "✓ Installed desktop file"

# Install PolicyKit policy
install -D -m 0644 assets/com.cpupowermanager.policy /usr/share/polkit-1/actions/com.cpupowermanager.policy
echo "✓ Installed PolicyKit policy"

# Install icon
install -D -m 0644 assets/icon.svg /usr/share/icons/hicolor/scalable/apps/cpu-power-manager.svg
echo "✓ Installed icon"

# Install systemd service (optional)
if [ -d "/etc/systemd/system" ]; then
    install -D -m 0644 assets/cpu-power-manager.service /etc/systemd/system/cpu-power-manager.service
    echo "✓ Installed systemd service"
    echo "  To enable: sudo systemctl enable cpu-power-manager"
    echo "  To start:  sudo systemctl start cpu-power-manager"
fi

# Update caches
if command -v gtk-update-icon-cache &> /dev/null; then
    gtk-update-icon-cache /usr/share/icons/hicolor/ 2>/dev/null || true
    echo "✓ Updated icon cache"
fi

if command -v update-desktop-database &> /dev/null; then
    update-desktop-database /usr/share/applications 2>/dev/null || true
    echo "✓ Updated desktop database"
fi

echo
echo "✓ Installation complete!"
echo
echo "You can now run 'cpu-power-manager' or find it in your application menu."
