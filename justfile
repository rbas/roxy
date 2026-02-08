# Roxy Justfile - Development task automation

# Default recipe (show available commands)
default:
    @just --list

# Run all tests
test:
    cargo test

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Run formatting and linting checks (CI equivalent)
check: fmt-check clippy
    @echo "✓ All checks passed!"

# Check code formatting
fmt-check:
    cargo fmt --check

# Run clippy lints
clippy:
    cargo clippy -- -D warnings

# Auto-fix formatting and clippy warnings
fix:
    cargo fmt
    cargo clippy --fix --allow-dirty --allow-staged

# Build optimized release binary
build-release:
    cargo build --release

# Run roxy daemon (requires installation)
run:
    cargo run

# Preview unreleased changelog entries
changelog:
    git cliff --unreleased

# Generate full CHANGELOG.md
changelog-full:
    git cliff -o CHANGELOG.md

# Create a new release (usage: just release patch|minor|major|0.2.0)
# Supports semantic version levels or explicit versions
# Also accepts v-prefixed versions (v0.2.0) for backward compatibility
release VERSION:
    #!/usr/bin/env bash
    set -euo pipefail

    # Strip 'v' prefix if present (backward compatibility)
    VERSION_CLEAN=$(echo "{{VERSION}}" | sed 's/^v//')

    # Check if cargo-release is installed
    if ! command -v cargo-release &> /dev/null; then
        echo "Error: cargo-release is not installed"
        echo "Install with: cargo install cargo-release"
        echo "Or run: just install-deps"
        exit 1
    fi

    # Check if git-cliff is installed
    if ! command -v git-cliff &> /dev/null; then
        echo "Error: git-cliff is not installed"
        echo "Install with: cargo install git-cliff"
        echo "Or run: just install-deps"
        exit 1
    fi

    # Ensure working directory is clean
    if [ -n "$(git status --porcelain)" ]; then
        echo "Error: Working directory is not clean"
        echo "Commit or stash changes first:"
        echo ""
        git status --short
        exit 1
    fi

    echo "Preparing release: $VERSION_CLEAN"
    echo ""

    # Show dry-run first
    echo "=== DRY RUN (showing what will happen) ==="
    echo "Note: Warning about 'Unrendered {{{{version}}}}' is expected in preview mode"
    echo ""
    cargo release "$VERSION_CLEAN" --no-push --no-publish --no-tag

    # Revert any CHANGELOG.md changes from the dry-run hook
    if [ -n "$(git status --porcelain CHANGELOG.md)" ]; then
        git checkout CHANGELOG.md
    fi

    echo ""
    echo "=== END DRY RUN ==="
    echo ""

    # Prompt for confirmation
    read -p "Proceed with release? This will create a commit and tag. (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Release cancelled"
        exit 1
    fi

    # Execute release (creates commit + tag, but doesn't push)
    echo ""
    echo "Creating release..."
    cargo release "$VERSION_CLEAN" --no-push --no-publish --execute

    # Get the tag that was just created
    TAG=$(git describe --tags --abbrev=0)

    echo ""
    echo "✓ Release $TAG created successfully!"
    echo ""
    echo "Next steps:"
    echo "  1. Review the commit: git show HEAD"
    echo "  2. Review the changelog: cat CHANGELOG.md"
    echo "  3. Push changes: git push && git push origin $TAG"
    echo ""
    echo "After pushing, GitHub Actions will automatically:"
    echo "  - Run tests and clippy"
    echo "  - Build release binary"
    echo "  - Create GitHub release with artifacts"

# Preview what a release would do without making changes
release-preview VERSION:
    @echo "Previewing release {{VERSION}}..."
    @echo "Note: Warning about 'Unrendered {{{{version}}}}' is expected in preview mode"
    @echo ""
    cargo release {{VERSION}} --no-push --no-publish

# Convenience: Create a patch release (most common)
release-patch:
    just release patch

# Convenience: Create a minor release
release-minor:
    just release minor

# Run security audit
audit:
    cargo audit

# Clean build artifacts
clean:
    cargo clean

# Install development dependencies
install-deps:
    @echo "Installing development dependencies..."
    @echo ""
    @echo "Installing cargo-audit..."
    cargo install cargo-audit
    @echo ""
    @echo "Installing cargo-release..."
    cargo install cargo-release
    @echo ""
    @echo "Installing git-cliff..."
    cargo install git-cliff
    @echo ""
    @echo "✓ All dependencies installed!"
