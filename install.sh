#!/bin/sh
# Engram 1-Click Installer for Unix / Linux / macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/code-amalps/Engram/main/install.sh | sh

set -eu

REPO="code-amalps/Engram"
VERSION="${ENGRAM_VERSION:-latest}"
AUTH_TOKEN="${GITHUB_TOKEN:-}"
INSTALL_DIR_OVERRIDE=""

# Parse CLI flags
while [ $# -gt 0 ]; do
  case "$1" in
    --tag|--version)
      VERSION="$2"
      shift 2
      ;;
    --tag=*|--version=*)
      VERSION="${1#*=}"
      shift 1
      ;;
    --token|--auth)
      AUTH_TOKEN="$2"
      shift 2
      ;;
    --token=*|--auth=*)
      AUTH_TOKEN="${1#*=}"
      shift 1
      ;;
    --dir|--install-dir)
      INSTALL_DIR_OVERRIDE="$2"
      shift 2
      ;;
    --dir=*|--install-dir=*)
      INSTALL_DIR_OVERRIDE="${1#*=}"
      shift 1
      ;;
    --repo)
      REPO="$2"
      shift 2
      ;;
    --repo=*)
      REPO="${1#*=}"
      shift 1
      ;;
    --help|-h)
      echo "Usage: install.sh [--tag <version>] [--token <github_token>] [--dir <install_dir>]"
      exit 0
      ;;
    *)
      shift 1
      ;;
  esac
done

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
        print_error "Unsupported Linux architecture: $ARCH"
        print_error "Engram currently provides x86_64 musl binaries. Linux ARM64 is not currently supported."
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
if [ -n "$INSTALL_DIR_OVERRIDE" ]; then
  INSTALL_DIR="$INSTALL_DIR_OVERRIDE"
elif [ -n "${ENGRAM_INSTALL_DIR:-}" ]; then
  INSTALL_DIR="$ENGRAM_INSTALL_DIR"
elif [ -w "/usr/local/bin" ]; then
  INSTALL_DIR="/usr/local/bin"
else
  INSTALL_DIR="$HOME/.local/bin"
fi

mkdir -p "$INSTALL_DIR"

# 3. Resolve Release Metadata
TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'engram_install_XXXXXX')"
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

RELEASE_JSON="$TMP_DIR/release.json"
AUTH_HEADER=""
if [ -n "$AUTH_TOKEN" ]; then
  AUTH_HEADER="Authorization: Bearer $AUTH_TOKEN"
fi

if command -v curl >/dev/null 2>&1; then
  if [ -n "$AUTH_HEADER" ]; then
    HTTP_CODE="$(curl -sSL -H "User-Agent: Engram-Installer" -H "Accept: application/vnd.github.v3+json" -H "$AUTH_HEADER" -w "%{http_code}" "$API_URL" -o "$RELEASE_JSON")"
  else
    HTTP_CODE="$(curl -sSL -H "User-Agent: Engram-Installer" -H "Accept: application/vnd.github.v3+json" -w "%{http_code}" "$API_URL" -o "$RELEASE_JSON")"
  fi
elif command -v wget >/dev/null 2>&1; then
  if [ -n "$AUTH_HEADER" ]; then
    wget -q --header="User-Agent: Engram-Installer" --header="Accept: application/vnd.github.v3+json" --header="$AUTH_HEADER" -O "$RELEASE_JSON" "$API_URL" || true
  else
    wget -q --header="User-Agent: Engram-Installer" --header="Accept: application/vnd.github.v3+json" -O "$RELEASE_JSON" "$API_URL" || true
  fi
  HTTP_CODE="200"
else
  print_error "Neither curl nor wget is available. Please install curl."
  exit 1
fi

TAG="$(grep -m1 '"tag_name":' "$RELEASE_JSON" | cut -d '"' -f 4 || true)"
if [ -z "$TAG" ]; then
  print_error "Failed to retrieve release metadata from GitHub API."
  print_error "If '$REPO' is a private repository, export GITHUB_TOKEN before running."
  exit 1
fi

print_success "Found release: $TAG"

ARCHIVE_NAME="engram-${TAG}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE_NAME}"
SUMS_URL="https://github.com/${REPO}/releases/download/${TAG}/SHA256SUMS"

# Resolve API asset URLs for private repositories
get_asset_api_url() {
  _target="$1"
  awk -v target="$_target" '
    /"url":/ {
      u = $0
      sub(/.*"url": *"/, "", u)
      sub(/".*/, "", u)
    }
    /"name":/ {
      n = $0
      sub(/.*"name": *"/, "", n)
      sub(/".*/, "", n)
      if (n == target && u ~ /releases\/assets\//) {
        print u
        exit
      }
    }
  ' "$RELEASE_JSON"
}

ARCHIVE_API_URL="$(get_asset_api_url "$ARCHIVE_NAME")"
SUMS_API_URL="$(get_asset_api_url "SHA256SUMS")"

ARCHIVE_FILE="$TMP_DIR/$ARCHIVE_NAME"
SUMS_FILE="$TMP_DIR/SHA256SUMS"

print_msg "Downloading $ARCHIVE_NAME..."
if command -v curl >/dev/null 2>&1; then
  if [ -n "$AUTH_HEADER" ] && [ -n "$ARCHIVE_API_URL" ]; then
    # Authenticated GitHub API asset endpoint for private repositories
    curl -sSfL -H "$AUTH_HEADER" -H "Accept: application/octet-stream" -o "$ARCHIVE_FILE" "$ARCHIVE_API_URL"
    if [ -n "$SUMS_API_URL" ]; then
      curl -sSfL -H "$AUTH_HEADER" -H "Accept: application/octet-stream" -o "$SUMS_FILE" "$SUMS_API_URL" || true
    fi
  elif [ -n "$AUTH_HEADER" ]; then
    curl -sSfL -H "$AUTH_HEADER" -o "$ARCHIVE_FILE" "$DOWNLOAD_URL"
    curl -sSfL -H "$AUTH_HEADER" -o "$SUMS_FILE" "$SUMS_URL" || true
  else
    curl -sSfL -o "$ARCHIVE_FILE" "$DOWNLOAD_URL"
    curl -sSfL -o "$SUMS_FILE" "$SUMS_URL" || true
  fi
else
  wget -q -O "$ARCHIVE_FILE" "$DOWNLOAD_URL"
  wget -q -O "$SUMS_FILE" "$SUMS_URL" || true
fi

if [ ! -f "$ARCHIVE_FILE" ] || [ ! -s "$ARCHIVE_FILE" ]; then
  print_error "Download failed: Archive file was not downloaded or is empty."
  exit 1
fi

# 4. Fail-Closed Checksum Verification
if [ ! -f "$SUMS_FILE" ] || [ ! -s "$SUMS_FILE" ]; then
  print_error "Integrity verification failed: SHA256SUMS manifest was not found or is empty."
  print_error "Refusing to install unverified artifact."
  exit 1
fi

print_msg "Verifying SHA256 checksum (fail-closed)..."
EXPECTED_HASH="$(grep -E "[0-9a-fA-F]{64}\s+(\./)?${ARCHIVE_NAME}$" "$SUMS_FILE" | awk '{print $1}' || true)"

if [ -z "$EXPECTED_HASH" ]; then
  print_error "Integrity verification failed: Could not find checksum entry for '$ARCHIVE_NAME' in SHA256SUMS."
  print_error "Refusing to install unverified artifact."
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL_HASH="$(sha256sum "$ARCHIVE_FILE" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  ACTUAL_HASH="$(shasum -a 256 "$ARCHIVE_FILE" | awk '{print $1}')"
else
  print_error "Neither sha256sum nor shasum is available on this system."
  print_error "Refusing to install unverified binary without cryptographic tooling."
  exit 1
fi

if [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
  print_error "Integrity verification failed: SHA-256 checksum mismatch for $ARCHIVE_NAME!"
  print_error "Expected: $EXPECTED_HASH"
  print_error "Actual:   $ACTUAL_HASH"
  print_error "Refusing to install corrupted or tampered binary."
  exit 1
fi
print_success "Checksum verified ✓ ($ACTUAL_HASH)"

# 5. Staged Extraction & Smoke Test
print_msg "Extracting and verifying staged binary..."
tar -xzf "$ARCHIVE_FILE" -C "$TMP_DIR"

if [ -f "$TMP_DIR/engram" ]; then
  BIN_SRC="$TMP_DIR/engram"
else
  BIN_SRC="$(find "$TMP_DIR" -type f -name engram | head -n 1)"
fi

if [ -z "$BIN_SRC" ] || [ ! -f "$BIN_SRC" ]; then
  print_error "Could not find 'engram' binary in extracted archive."
  exit 1
fi

chmod +x "$BIN_SRC"

# Execute smoke-test on staged binary
"$BIN_SRC" --version >/dev/null 2>&1 || {
  print_error "Staged binary execution failed. Refusing to replace installed executable."
  exit 1
}

# 6. Safe Installation
DEST_BIN="$INSTALL_DIR/engram"
BACKUP_BIN="$INSTALL_DIR/engram.bak"

if [ -f "$DEST_BIN" ]; then
  cp -f "$DEST_BIN" "$BACKUP_BIN" 2>/dev/null || true
fi

if mv -f "$BIN_SRC" "$DEST_BIN"; then
  chmod +x "$DEST_BIN"
  rm -f "$BACKUP_BIN" 2>/dev/null || true
  print_success "Installed engram successfully to $DEST_BIN ✓"
else
  if [ -f "$BACKUP_BIN" ]; then
    mv -f "$BACKUP_BIN" "$DEST_BIN" 2>/dev/null || true
    print_error "Installation failed. Restored previous working binary."
  else
    print_error "Installation failed."
  fi
  exit 1
fi

# 7. Verify PATH
printf "──────────────────────────────────────────────────────\n"
"$DEST_BIN" --version

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    print_warn ""
    print_warn "Notice: $INSTALL_DIR is not in your current PATH."
    print_warn "Add it to your shell configuration (e.g. ~/.bashrc or ~/.zshrc):"
    printf "  export PATH=\"%s:\$PATH\"\n\n" "$INSTALL_DIR"
    ;;
esac

printf "──────────────────────────────────────────────────────\n"
print_success "🎉 Engram $TAG installed successfully!"
