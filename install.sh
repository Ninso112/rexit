#!/bin/sh

set -e

REPO_URL="https://github.com/Ninso112/rexit"
CARGO_BIN="${HOME}/.cargo/bin"
TARGET_BIN="/usr/local/bin"

echo "=========================================="
echo "     rexit - Installation Script"
echo "=========================================="
echo ""

# Check if cargo is installed
if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: Rust/Cargo is not installed."
    echo ""
    echo "Please install Rust first by running:"
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
    echo "Or visit: https://rustup.rs/"
    exit 1
fi

echo "✓ Rust/Cargo found"

# Check if rexit is already installed
if command -v rexit >/dev/null 2>&1; then
    echo "⚠ rexit is already installed. Updating..."
else
    echo "→ Installing rexit..."
fi

# Install from git repository
echo "→ Downloading and building rexit..."
if cargo install --git "$REPO_URL" --force; then
    echo ""
    echo "→ Copying binary to $TARGET_BIN..."

    # Try to copy to /usr/local/bin
    if sudo cp "$CARGO_BIN/rexit" "$TARGET_BIN/rexit" 2>/dev/null; then
        echo "✓ Binary installed to $TARGET_BIN/rexit"
        sudo chmod +x "$TARGET_BIN/rexit"
    else
        echo "⚠ Could not copy to $TARGET_BIN (sudo required)"
        echo "  Binary is available at: $CARGO_BIN/rexit"
        echo ""
        echo "To make it system-wide available, run:"
        echo "    sudo cp $CARGO_BIN/rexit $TARGET_BIN/"
    fi

    echo ""
    echo "=========================================="
    echo "     ✓ Installation successful!"
    echo "=========================================="
    echo ""
    echo "rexit is now available. Run it with:"
    echo "    rexit"
    echo ""
    echo "Generate default config with:"
    echo "    rexit --init"
    echo ""
    echo "For more information, visit:"
    echo "    $REPO_URL"
else
    echo ""
    echo "Error: Installation failed."
    exit 1
fi
