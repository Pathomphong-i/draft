#!/bin/sh
# ==============================================================================
# Draft VCS Universal Installer
# Usage: curl -fsSL https://draftmultiverse.org/install.sh | sh
# ==============================================================================
set -e

# Configuration
REPO="Pathomphong-i/draft"
VERSION="${DFT_VERSION:-latest}"
INSTALL_DIR="${DFT_INSTALL_DIR:-$HOME/.local/bin}"

# Formatting
BOLD='\033[1m'
CYAN='\033[0;36m'
VIOLET='\033[0;35m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

if [ ! -t 1 ]; then
    BOLD=""
    CYAN=""
    VIOLET=""
    GREEN=""
    RED=""
    NC=""
fi

echo "${VIOLET}"
echo "    ____             ________ "
echo "   / __ \_________ _/ __/ /_ "
echo "  / / / / ___/ __ \`/ /_/ __/ "
echo " / /_/ / /  / /_/ / __/ /_   "
echo "/_____/_/   \__,_/_/  \__/   "
echo "                             "
echo "${CYAN}The Multiverse Version Control System for AI Agent Swarms${NC}"
echo "---------------------------------------------------------"

# 1. Detect Operating System and Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin)
        OS_TYPE="apple-darwin"
        ;;
    Linux)
        OS_TYPE="unknown-linux-gnu"
        ;;
    *)
        echo "${RED}Error: Unsupported operating system: $OS${NC}" >&2
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64|amd64)
        ARCH_TYPE="x86_64"
        ;;
    arm64|aarch64)
        ARCH_TYPE="aarch64"
        ;;
    *)
        echo "${RED}Error: Unsupported processor architecture: $ARCH${NC}" >&2
        exit 1
        ;;
esac

TARGET="${ARCH_TYPE}-${OS_TYPE}"
echo "Platform detected: ${BOLD}${TARGET}${NC}"

# 2. Resolve download URL
if [ "$VERSION" = "latest" ]; then
    RELEASE_TAG="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || echo "")"
    if [ -z "$RELEASE_TAG" ]; then
        RELEASE_TAG="v0.1.0"
    fi
else
    RELEASE_TAG="$VERSION"
fi

TARBALL="dft-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${RELEASE_TAG}/${TARBALL}"
FALLBACK_URL="https://draftmultiverse.org/releases/${RELEASE_TAG}/${TARBALL}"

echo "Target release:    ${BOLD}${RELEASE_TAG}${NC}"
echo "Install path:      ${BOLD}${INSTALL_DIR}/dft${NC}"

# 3. Create destination directory
mkdir -p "$INSTALL_DIR"

# 4. Download and extract
TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'draft-install')"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Downloading binary package..."
if curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$TARBALL" 2>/dev/null; then
    :
elif curl -fsSL "$FALLBACK_URL" -o "$TMP_DIR/$TARBALL" 2>/dev/null; then
    :
else
    echo "${RED}Failed to download ${TARBALL} from release mirrors.${NC}" >&2
    echo "You can build directly from source via Cargo:" >&2
    echo "  cargo install --git https://github.com/${REPO}.git daft-cli" >&2
    exit 1
fi

echo "Extracting binary..."
tar -xzf "$TMP_DIR/$TARBALL" -C "$TMP_DIR"

if [ -f "$TMP_DIR/dft" ]; then
    BIN_SRC="$TMP_DIR/dft"
elif [ -f "$TMP_DIR/target/release/dft" ]; then
    BIN_SRC="$TMP_DIR/target/release/dft"
else
    BIN_SRC="$(find "$TMP_DIR" -type f -name 'dft' | head -n 1)"
fi

if [ -z "$BIN_SRC" ] || [ ! -f "$BIN_SRC" ]; then
    echo "${RED}Error: 'dft' binary not found in downloaded package.${NC}" >&2
    exit 1
fi

cp "$BIN_SRC" "$INSTALL_DIR/dft"
chmod +x "$INSTALL_DIR/dft"

# 5. Create symlinks for 'draft' and 'drf'
ln -sf "$INSTALL_DIR/dft" "$INSTALL_DIR/draft"
ln -sf "$INSTALL_DIR/dft" "$INSTALL_DIR/drf"

echo "${GREEN}✔ Draft successfully installed to ${INSTALL_DIR}/dft!${NC}"

# 6. Verify PATH
case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        ;;
    *)
        echo ""
        echo "${BOLD}Notice:${NC} '${INSTALL_DIR}' is not in your current PATH."
        echo "Add it to your shell configuration to run 'dft' or 'draft' from anywhere:"
        echo ""
        if [ -n "$ZSH_VERSION" ] || [ -f "$HOME/.zshrc" ]; then
            echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc && source ~/.zshrc"
        elif [ -n "$BASH_VERSION" ] || [ -f "$HOME/.bashrc" ]; then
            echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc && source ~/.bashrc"
        elif [ -f "$HOME/.config/fish/config.fish" ]; then
            echo "  fish_add_path $INSTALL_DIR"
        else
            echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        fi
        echo ""
        ;;
esac

echo "Run ${BOLD}dft --help${NC} or launch the web multiverse with ${BOLD}dft ui${NC}."
