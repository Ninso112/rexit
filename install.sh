#!/bin/sh

set -e

REPO_URL="https://github.com/Ninso112/rexit"
CARGO_BIN="${HOME}/.cargo/bin"
TARGET_BIN="/usr/local/bin"
CONFIG_DIR="${HOME}/.config/rexit"
THEMES_DIR="${CONFIG_DIR}/themes"
MAN_DIR="/usr/local/share/man/man1"

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

    # Create config directory
    echo "→ Creating config directory..."
    mkdir -p "$THEMES_DIR"
    echo "✓ Config directory created at $CONFIG_DIR"

    # Install themes if they exist in the repository
    if [ -d "assets" ]; then
        echo "→ Installing themes..."
        for theme in assets/*.toml; do
            if [ -f "$theme" ]; then
                cp "$theme" "$THEMES_DIR/"
                echo "  ✓ Installed $(basename "$theme")"
            fi
        done
        echo "✓ Themes installed to $THEMES_DIR"
    fi

    # Install man page if possible
    echo "→ Installing man page..."
    if [ -f "assets/rexit.1" ]; then
        if sudo cp "assets/rexit.1" "$MAN_DIR/rexit.1" 2>/dev/null; then
            sudo chmod 644 "$MAN_DIR/rexit.1"
            if command -v mandb >/dev/null 2>&1; then
                sudo mandb >/dev/null 2>&1
            fi
            echo "✓ Man page installed to $MAN_DIR/rexit.1"
        else
            echo "⚠ Could not install man page (sudo required)"
            echo "  To install manually, run:"
            echo "    sudo cp assets/rexit.1 $MAN_DIR/"
        fi
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
    echo "List available themes with:"
    echo "    rexit --list-themes"
    echo ""
    echo "Use a theme with:"
    echo "    rexit --theme dracula"
    echo ""
    echo "View the man page with:"
    echo "    man rexit"
    echo ""
    echo "For more information, visit:"
    echo "    $REPO_URL"
else
    echo ""
    echo "Error: Installation failed."
    exit 1
fi
