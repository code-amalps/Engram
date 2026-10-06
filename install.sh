#!/bin/sh
# Engram 1-Click Installer for Unix / Linux / macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/code-amalps/Engram/main/install.sh | sh

set -eu

REPO="code-amalps/Engram"
VERSION="${ENGRAM_VERSION:-latest}"

print_msg() {
  printf "\033[1;36m%s\033[0m\n" "$1"
}

print_success() {
  printf "\033[1;32m%s\033[0m\n" "$1"
}

print_warn() {
  printf "\033[1;33m%s\033[0m\n" "$1"
}

print_error() {
  printf "\033[1;31m%s\033[0m\n" "$1" >&2
}

print_msg "🧠 Engram Installer for Linux / macOS"
printf "──────────────────────────────────────────────────────\n"

# 1. Detect OS & Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64|amd64) TARGET="x86_64-unknown-linux-musl" ;;
      *)
        print_error "Unsupported Linux architecture: $ARCH (Engram currently provides x86_64 musl binaries)"
        exit 1
        ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
      x86_64|amd64) TARGET="x86_64-apple-darwin" ;;
      *)
        print_error "Unsupported macOS architecture: $ARCH"
        exit 1
        ;;
    esac
    ;;
  *)
    print_error "Unsupported operating system: $OS"
    exit 1
    ;;
esac

# 2. Determine Install Directory
if [ -n "${ENGRAM_INSTALL_DIR:-}" ]; then
  INSTALL_DIR="$ENGRAM_INSTALL_DIR"
elif [ -w "/usr/local/bin" ]; then
  INSTALL_DIR="/usr/local/bin"
else
  INSTALL_DIR="$HOME/.engram/bin"
fi

mkdir -p "$INSTALL_DIR"

# 3. Resolve Release
TMP_DIR="$(mktemp -d -t engram_install_XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [ "$VERSION" = "latest" ]; then
  API_URL="https://api.github.com/repos/$REPO/releases/latest"
else
  TAG_NAME="$VERSION"
  case "$TAG_NAME" in
    v*) ;;
    *) TAG_NAME="v$VERSION" ;;
  esac
  API_URL="https://api.github.com/repos/$REPO/releases/tags/$TAG_NAME"
fi

print_msg "Resolving release ($VERSION) for $TARGET..."

DOWNLOAD_CMD=""
if command -v curl >/dev/null 2>&1; then
  DOWNLOAD_CMD="curl -fsSL"
elif command -v wget >/dev/null 2>&1; then
  DOWNLOAD_CMD="wget -qO-"
else
  print_error "Neither curl nor wget is available. Please install curl."
  exit 1
fi

RELEASE_JSON="$TMP_DIR/release.json"
if command -v curl >/dev/null 2>&1; then
  curl -sSL -H "User-Agent: Engram-Installer" "$API_URL" -o "$RELEASE_JSON"
else
  wget -q -U "Engram-Installer" -O "$RELEASE_JSON" "$API_URL"
fi

TAG="$(grep -m1 '"tag_name":' "$RELEASE_JSON" | cut -d '"' -f 4 || true)"
if [ -z "$TAG" ]; then
  print_error "Failed to retrieve release metadata from $API_URL"
  exit 1
fi

print_success "Found release: $TAG"

ARCHIVE_NAME="engram-${TAG}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE_NAME}"
SUMS_URL="https://github.com/${REPO}/releases/download/${TAG}/SHA256SUMS"

ARCHIVE_FILE="$TMP_DIR/$ARCHIVE_NAME"
SUMS_FILE="$TMP_DIR/SHA256SUMS"

print_msg "Downloading $ARCHIVE_NAME..."
if command -v curl >/dev/null 2>&1; then
  curl -fL -o "$ARCHIVE_FILE" "$DOWNLOAD_URL"
  curl -fsL -o "$SUMS_FILE" "$SUMS_URL" || true
else
  wget -q -O "$ARCHIVE_FILE" "$DOWNLOAD_URL"
  wget -q -O "$SUMS_FILE" "$SUMS_URL" || true
fi

# 4. Checksum Verification
if [ -f "$SUMS_FILE" ]; then
  print_msg "Verifying SHA256 checksum..."
  EXPECTED_HASH="$(grep "$ARCHIVE_NAME" "$SUMS_FILE" | awk '{print $1}' || true)"
  if [ -n "$EXPECTED_HASH" ]; then
    if command -v sha256sum >/dev/null 2>&1; then
      ACTUAL_HASH="$(sha256sum "$ARCHIVE_FILE" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
      ACTUAL_HASH="$(shasum -a 256 "$ARCHIVE_FILE" | awk '{print $1}')"
    else
      ACTUAL_HASH=""
      print_warn "Neither sha256sum nor shasum available; skipping checksum verification."
    fi

    if [ -n "$ACTUAL_HASH" ]; then
      if [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
        print_error "Checksum verification failed!"
        print_error "Expected: $EXPECTED_HASH"
        print_error "Actual:   $ACTUAL_HASH"
        exit 1
      fi
      print_success "Checksum verified ✓ ($ACTUAL_HASH)"
    fi
  else
    print_warn "Archive not found in SHA256SUMS, proceeding with install."
  fi
fi

# 5. Extract and Install
print_msg "Installing engram to $INSTALL_DIR..."
tar -xzf "$ARCHIVE_FILE" -C "$TMP_DIR"

if [ -f "$TMP_DIR/engram" ]; then
  BIN_SRC="$TMP_DIR/engram"
elif [ -f "$TMP_DIR/target/release/engram" ]; then
  BIN_SRC="$TMP_DIR/target/release/engram"
else
  BIN_SRC="$(find "$TMP_DIR" -type f -name engram | head -n 1)"
fi

if [ -z "$BIN_SRC" ] || [ ! -f "$BIN_SRC" ]; then
  print_error "Could not find 'engram' binary in extracted archive."
  exit 1
fi

DEST_BIN="$INSTALL_DIR/engram"
mv "$BIN_SRC" "$DEST_BIN"
chmod +x "$DEST_BIN"

print_success "Installed engram successfully ✓"

# 6. Verify PATH & Smoke Test
printf "──────────────────────────────────────────────────────\n"
print_msg "Verifying installation..."
"$DEST_BIN" --version

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    print_warn ""
    print_warn "Notice: $INSTALL_DIR is not in your current PATH."
    print_warn "Add it to your shell configuration file (e.g. ~/.bashrc, ~/.zshrc):"
    printf "  export PATH=\"%s:\$PATH\"\n\n" "$INSTALL_DIR"
    ;;
esac

printf "──────────────────────────────────────────────────────\n"
print_success "🎉 Engram $TAG installed successfully!"
printf "To get started in any repository, run:\n"
printf "  engram init\n"
printf "  engram setup cursor  # or claude, windsurf\n"
printf "  engram bench\n"
