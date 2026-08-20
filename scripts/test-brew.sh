#!/usr/bin/env bash
#
# Test the Homebrew formula and Roxy's managed service locally.
#
# Usage:
#   ./scripts/test-brew.sh          # build + install + show next steps
#   ./scripts/test-brew.sh clean    # uninstall and restore tap
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TAP_DIR="$(brew --prefix)/Library/Taps/rbas/homebrew-roxy"
TARBALL="/tmp/roxy-local-test.tar.gz"
VERSION=$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')

# ── Clean mode ─────────────────────────────────────────────
if [[ "${1:-}" == "clean" ]]; then
    echo "==> Cleaning up test install..."
    sudo roxy uninstall --force 2>/dev/null || true
    brew uninstall roxy 2>/dev/null || true

    # Restore tap from remote
    if [[ -d "$TAP_DIR" ]]; then
        echo "==> Restoring tap to remote version..."
        cd "$TAP_DIR"
        git checkout Formula/roxy.rb 2>/dev/null || true
    fi

    # Clean up roxy state
    rm -f "$TARBALL"

    echo "Done. Tap restored, roxy uninstalled."
    exit 0
fi

# ── Build ──────────────────────────────────────────────────
echo "==> Building release binary..."
cd "$REPO_ROOT"
cargo build --release

echo "==> Creating tarball..."
tar -czf "$TARBALL" -C target/release roxy

SHA256=$(shasum -a 256 "$TARBALL" | awk '{print $1}')
echo "    tarball: $TARBALL"
echo "    sha256:  $SHA256"
echo "    version: $VERSION"

# ── Ensure tap exists ──────────────────────────────────────
if [[ ! -d "$TAP_DIR" ]]; then
    echo "==> Tapping rbas/roxy..."
    brew tap rbas/roxy
fi

# ── Write local formula ───────────────────────────────────
echo "==> Writing local formula to tap..."
mkdir -p "$TAP_DIR/Formula"

sed -e "s|__VERSION__|$VERSION|g" \
    -e "s|__URL__|file://$TARBALL|g" \
    -e "s|__SHA256__|$SHA256|g" \
    "$REPO_ROOT/scripts/formula.rb.template" \
    > "$TAP_DIR/Formula/roxy.rb"

# ── Install ────────────────────────────────────────────────
echo "==> Installing roxy from local tap..."
brew reinstall roxy

echo ""
echo "============================================"
echo "  Roxy installed from local build!"
echo "  Version: $(roxy --version)"
echo "============================================"
echo ""
echo "Now test the full flow:"
echo ""
echo "  # 1. One-time setup"
echo "  sudo roxy install"
echo ""
echo "  # 2. Register a test domain"
echo "  roxy register test.roxy --route '/=8080'"
echo ""
echo "  # 3. Test the automatically installed user service"
echo "  roxy status"
echo "  roxy stop"
echo "  roxy start"
echo ""
echo "  # 4. Verify config location"
echo "  cat \"$HOME/Library/Application Support/Roxy/config.toml\""
echo "  ls -la \"$HOME/Library/Application Support/Roxy/\""
echo ""
echo "  # 5. Clean up when done"
echo "  ./scripts/test-brew.sh clean"
