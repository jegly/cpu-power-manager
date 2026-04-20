#!/bin/bash
# Build script for CPU Power Manager

set -e

echo "=== CPU Power Manager Build Script by JEGLY ==="
echo

# Check for required tools
check_command() {
    if ! command -v $1 &> /dev/null; then
        echo "ERROR: $1 is not installed"
        echo "Please install it with: sudo apt install $2"
        exit 1
    fi
}

echo "Checking dependencies..."
check_command cargo "cargo"
check_command rustc "rustc"
check_command pkg-config "pkg-config"

# Check for GTK4
if ! pkg-config --exists gtk4; then
    echo "ERROR: GTK4 development files not found"
    echo "Please install: sudo apt install libgtk-4-dev libadwaita-1-dev"
    exit 1
fi

echo "✓ All dependencies found"
echo

# Build
echo "Building release version..."
cargo build --release

if [ $? -eq 0 ]; then
    echo
    echo "✓ Build successful!"
    echo
    echo "Binary location: target/release/cpu-power-manager"
    echo
    echo "To install system-wide, run:"
    echo "  sudo ./install.sh"
    echo
    echo "To create a .deb package, run:"
    echo "  cargo install cargo-deb"
    echo "  cargo deb"
else
    echo
    echo "✗ Build failed"
    exit 1
fi
