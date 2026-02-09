#!/usr/bin/env bash
#
# Test Homebrew formula locally with brew services.
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
    sudo brew services stop roxy 2>/dev/null || true
    brew uninstall roxy 2>/dev/null || true

    # Restore tap from remote
    if [[ -d "$TAP_DIR" ]]; then
        echo "==> Restoring tap to remote version..."
        cd "$TAP_DIR"
        git checkout Formula/roxy.rb 2>/dev/null || true
    fi

    # Clean up roxy state
    sudo roxy uninstall --force 2>/dev/null || true
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
echo "  sudo roxy register test.roxy --route '/=8080'"
echo ""
echo "  # 3a. Test manual start"
echo "  sudo roxy start"
echo "  roxy status"
echo "  sudo roxy stop"
echo ""
echo "  # 3b. Test brew services (auto-start at boot)"
echo "  sudo brew services start roxy"
echo "  sudo brew services info roxy"
echo "  roxy status"
echo "  sudo brew services stop roxy"
echo ""
echo "  # 4. Verify config location"
echo "  cat /etc/roxy/config.toml"
echo "  ls -la /etc/roxy/"
echo ""
echo "  # 5. Clean up when done"
echo "  ./scripts/test-brew.sh clean"
