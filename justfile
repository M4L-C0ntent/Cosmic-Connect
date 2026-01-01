# justfile
# justfile for cosmic-connect-applet

# Variables
flatpak_manifest := "io.github.M4LC0ntent.CosmicConnect.json"
flatpak_id := "io.github.M4LC0ntent.CosmicConnect"
flatpak_build_dir := ".flatpak-builder"
flatpak_repo_dir := "flatpak-repo"

# Default recipe to display available commands
default:
    @just --list

# ================================
# Standard Build Commands
# ================================

# Build all binaries in debug mode
build:
    cargo build

# Build all binaries in release mode
build-release:
    cargo build --release

# Build a specific binary in debug mode
build-bin BIN:
    cargo build --bin {{BIN}}

# Build a specific binary in release mode
build-bin-release BIN:
    cargo build --release --bin {{BIN}}

# Run the applet in debug mode
run-applet:
    cargo run --bin cosmic-connect-applet

# Run the settings app in debug mode
run-settings:
    cargo run --bin cosmic-connect-settings

# Run the SMS app in debug mode
run-sms:
    cargo run --bin cosmic-connect-sms

# Run a specific binary in release mode
run-release BIN:
    cargo run --release --bin {{BIN}}

# Clean build artifacts
clean:
    cargo clean
    rm -rf {{flatpak_build_dir}} {{flatpak_repo_dir}} .flatpak-venv

# Clean only cargo artifacts
clean-cargo:
    cargo clean

# Clean only flatpak artifacts
clean-flatpak:
    rm -rf {{flatpak_build_dir}} {{flatpak_repo_dir}} .flatpak-venv

# Check code without building
check:
    cargo check

# Format code using rustfmt
fmt:
    cargo fmt

# Check code formatting
fmt-check:
    cargo fmt -- --check

# Run clippy lints
clippy:
    cargo clippy -- -D warnings

# Run all checks (fmt, clippy, build)
test-all: fmt-check clippy build
    @echo "✓ All checks passed!"

# ================================
# Standard Installation Commands
# ================================

# Install binaries to /usr/local/bin (run with: sudo just install-bins)
install-bins:
    install -Dm755 target/release/cosmic-connect-applet /usr/local/bin/cosmic-connect-applet
    install -Dm755 target/release/cosmic-connect-settings /usr/local/bin/cosmic-connect-settings
    install -Dm755 target/release/cosmic-connect-sms /usr/local/bin/cosmic-connect-sms
    @echo "✓ Binaries installed to /usr/local/bin"

# Install desktop files to /usr/share/applications (run with: sudo just install-desktop)
install-desktop:
    install -Dm644 data/io.github.M4LC0ntent.CosmicConnect.desktop /usr/share/applications/io.github.M4LC0ntent.CosmicConnect.desktop
    install -Dm644 data/io.github.M4LC0ntent.CosmicConnectSettings.desktop /usr/share/applications/io.github.M4LC0ntent.CosmicConnectSettings.desktop
    install -Dm644 data/io.github.M4LC0ntent.CosmicConnectSMS.desktop /usr/share/applications/io.github.M4LC0ntent.CosmicConnectSMS.desktop
    update-desktop-database /usr/share/applications/ 2>/dev/null || true
    @echo "✓ Desktop files installed"

# Register as handler for kdeconnect:// URLs
register-handler:
    xdg-mime default io.github.M4LC0ntent.CosmicConnectSettings.desktop x-scheme-handler/kdeconnect
    @echo "Checking handler registration..."
    @HANDLER=$(xdg-mime query default x-scheme-handler/kdeconnect); \
    if [ "$$HANDLER" = "io.github.M4LC0ntent.CosmicConnectSettings.desktop" ]; then \
        echo "✓ Successfully registered as pairing notification handler"; \
    else \
        echo "⚠ Handler registration may have failed. Current: $$HANDLER"; \
    fi

# Restart cosmic panel
restart-panel:
    @echo "Restarting cosmic-panel..."
    killall cosmic-panel || true
    @echo "✓ Panel restarted"

# Full installation: install binaries, install desktop files, register handler, disable KDE notifications, restart panel (run with: sudo just install)
install: install-bins install-desktop
    @echo ""
    @./scripts/register-handler.sh
    @echo ""
    @./scripts/disable_kde_notifications.sh
    @just restart-panel
    @echo ""
    @echo "✓ Installation complete!"

# Quick install: only install binaries and restart panel (assumes already built, run with: sudo just install-quick)
install-quick: install-bins restart-panel
    @echo "✓ Quick install complete!"

# Uninstall all binaries and desktop files (run with: sudo just uninstall)
uninstall:
    rm -f /usr/local/bin/cosmic-connect-applet
    rm -f /usr/local/bin/cosmic-connect-settings
    rm -f /usr/local/bin/cosmic-connect-sms
    rm -f /usr/share/applications/io.github.M4LC0ntent.CosmicConnect.desktop
    rm -f /usr/share/applications/io.github.M4LC0ntent.CosmicConnectSettings.desktop
    rm -f /usr/share/applications/io.github.M4LC0ntent.CosmicConnectSMS.desktop
    update-desktop-database /usr/share/applications/ 2>/dev/null || true
    @echo "✓ Uninstalled successfully"

# Update from git and reinstall (run with: sudo just update)
update:
    git pull
    cargo build --release
    just install

# Check system dependencies
check-deps:
    @./scripts/check-deps.sh

# Install missing system dependencies (run with: sudo just install-deps)
install-deps:
    @./scripts/install-deps.sh

# ================================
# Flatpak Commands
# ================================

# Check if flatpak-builder is installed
check-flatpak-builder:
    @which flatpak-builder > /dev/null || (echo "Error: flatpak-builder not found. Install with: sudo apt install flatpak-builder" && exit 1)

# Install required flatpak runtimes and SDK extensions
install-flatpak-runtimes:
    @echo "Installing required flatpak runtimes and extensions..."
    @echo "Installing Freedesktop Platform and SDK 24.08..."
    flatpak install --user -y flathub org.freedesktop.Platform//24.08 || true
    flatpak install --user -y flathub org.freedesktop.Sdk//24.08 || true
    @echo "Installing Rust stable extension..."
    flatpak install --user -y flathub org.freedesktop.Sdk.Extension.rust-stable//24.08 || true
    @echo "✓ Flatpak runtimes and extensions installed"

# Check if required flatpak runtimes are installed
check-flatpak-runtimes:
    @echo "Checking flatpak runtimes..."
    @if ! flatpak list --runtime | grep -q "org.freedesktop.Platform.*24.08"; then \
        echo "Error: org.freedesktop.Platform/24.08 not installed"; \
        echo "Run: just install-flatpak-runtimes"; \
        exit 1; \
    fi
    @if ! flatpak list --runtime | grep -q "org.freedesktop.Sdk.*24.08"; then \
        echo "Error: org.freedesktop.Sdk/24.08 not installed"; \
        echo "Run: just install-flatpak-runtimes"; \
        exit 1; \
    fi
    @if ! flatpak list --runtime | grep -q "org.freedesktop.Sdk.Extension.rust-stable.*24.08"; then \
        echo "Error: org.freedesktop.Sdk.Extension.rust-stable/24.08 not installed"; \
        echo "Run: just install-flatpak-runtimes"; \
        exit 1; \
    fi
    @echo "✓ All required runtimes are installed"

# Install Python dependencies for flatpak-cargo-generator
install-flatpak-python-deps:
    @echo "Installing Python dependencies for flatpak generation..."
    @echo "Attempting to install system packages first..."
    @if sudo apt install -y python3-aiohttp python3-tomlkit 2>/dev/null; then \
        echo "✓ Python dependencies installed from system packages"; \
    else \
        echo "System packages not available, creating virtual environment..."; \
        python3 -m venv .flatpak-venv || (echo "Error: python3-venv not installed. Run: sudo apt install python3-venv" && exit 1); \
        .flatpak-venv/bin/pip install aiohttp tomlkit; \
        echo "✓ Python dependencies installed in virtual environment (.flatpak-venv)"; \
        echo "Note: flatpak-gen-sources will automatically use the venv"; \
    fi

# Generate cargo sources for flatpak (requires flatpak-cargo-generator.py)
flatpak-gen-sources:
    @echo "Generating cargo sources for flatpak..."
    @if [ ! -f "flatpak-cargo-generator.py" ]; then \
        echo "Downloading flatpak-cargo-generator.py..."; \
        wget https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py; \
        chmod +x flatpak-cargo-generator.py; \
    fi
    @echo "Checking Python dependencies..."
    @if python3 -c "import aiohttp, tomlkit" 2>/dev/null; then \
        echo "Using system Python packages..."; \
        python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json; \
    elif [ -d ".flatpak-venv" ]; then \
        echo "Using virtual environment..."; \
        .flatpak-venv/bin/python flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json; \
    else \
        echo "Error: Python dependencies not found."; \
        echo "Run: just install-flatpak-python-deps"; \
        exit 1; \
    fi
    @echo "Generating complete flatpak manifest..."
    python3 generate-flatpak-manifest.py io.github.M4LC0ntent.CosmicConnect.base.json cargo-sources.json {{flatpak_manifest}}
    @echo "✓ Generated complete manifest: {{flatpak_manifest}}"

# Build flatpak
flatpak-build: check-flatpak-builder check-flatpak-runtimes
    @echo "Building flatpak..."
    flatpak-builder --user --install --force-clean --disable-rofiles-fuse {{flatpak_build_dir}} {{flatpak_manifest}}
    @echo "✓ Flatpak built and installed"
    @echo ""
    @echo "RECOMMENDED: Disable KDE Connect notifications to prevent duplicates"
    @echo "This script modifies KDE Connect config files on your host system."
    @echo "Run: just flatpak-disable-kde-notifications"

# Build flatpak with cargo sources generation
flatpak-build-full: flatpak-gen-sources flatpak-build

# Build flatpak to a repository (for distribution)
flatpak-build-repo: check-flatpak-builder check-flatpak-runtimes
    @echo "Building flatpak to repository..."
    flatpak-builder --repo={{flatpak_repo_dir}} --force-clean --disable-rofiles-fuse {{flatpak_build_dir}} {{flatpak_manifest}}
    @echo "✓ Flatpak built to repository: {{flatpak_repo_dir}}"

# Export flatpak as a single-file bundle
flatpak-bundle: flatpak-build-repo
    @echo "Creating flatpak bundle..."
    flatpak build-bundle {{flatpak_repo_dir}} {{flatpak_id}}.flatpak {{flatpak_id}}
    @echo "✓ Bundle created: {{flatpak_id}}.flatpak"

# Install flatpak from local repository
flatpak-install-repo: flatpak-build-repo
    @echo "Installing flatpak from repository..."
    flatpak --user remote-add --if-not-exists --no-gpg-verify cosmic-connect-repo {{flatpak_repo_dir}}
    flatpak --user install -y cosmic-connect-repo {{flatpak_id}}
    @echo "✓ Flatpak installed from repository"

# Install flatpak bundle
flatpak-install-bundle:
    @echo "Installing flatpak bundle..."
    @if [ ! -f "{{flatpak_id}}.flatpak" ]; then \
        echo "Error: {{flatpak_id}}.flatpak not found. Run 'just flatpak-bundle' first."; \
        exit 1; \
    fi
    flatpak --user install -y {{flatpak_id}}.flatpak
    @echo "✓ Flatpak bundle installed"

# Run flatpak applet
flatpak-run:
    flatpak run {{flatpak_id}}

# Run flatpak settings
flatpak-run-settings:
    flatpak run --command=cosmic-connect-settings {{flatpak_id}}

# Run flatpak SMS app
flatpak-run-sms:
    flatpak run --command=cosmic-connect-sms {{flatpak_id}}

# Uninstall flatpak
flatpak-uninstall:
    @echo "Uninstalling flatpak..."
    flatpak --user uninstall -y {{flatpak_id}} || true
    flatpak --user remote-delete cosmic-connect-repo || true
    @echo "✓ Flatpak uninstalled"

# Rebuild flatpak (clean and build)
flatpak-rebuild: clean-flatpak flatpak-build

# Show flatpak info
flatpak-info:
    flatpak info {{flatpak_id}}

# List flatpak files
flatpak-list:
    flatpak list --app | grep -i cosmic

# Check flatpak permissions
flatpak-permissions:
    flatpak info --show-permissions {{flatpak_id}}

# Override flatpak permissions (interactive)
flatpak-override:
    flatpak override --user {{flatpak_id}}

# ================================
# Utility Commands
# ================================

# Disable KDE notifications (for flatpak)
flatpak-disable-kde-notifications:
    @echo "=========================================="
    @echo "WARNING: This script will modify KDE Connect configuration on your HOST system"
    @echo ""
    @echo "Changes:"
    @echo "  - Modifies ~/.config/kdeconnect.notifyrc"
    @echo "  - Modifies ~/.config/kdeconnectrc"
    @echo "  - Modifies device configs in ~/.config/kdeconnect/"
    @echo "  - Restarts kdeconnectd daemon"
    @echo ""
    @echo "Purpose: Prevents duplicate notifications (KDE Connect + Cosmic Connect)"
    @echo ""
    @echo "Requirements:"
    @echo "  - kwriteconfig5 must be installed (package: kde-cli-tools)"
    @echo ""
    @echo "To revert: Run 'just flatpak-enable-kde-notifications'"
    @echo "=========================================="
    @echo ""
    @bash -c 'read -p "Continue? (y/N) " confirm; \
    if [ "$$confirm" != "y" ] && [ "$$confirm" != "Y" ]; then \
        echo "Cancelled"; \
        exit 1; \
    fi'
    @echo ""
    @echo "Extracting and running disable_kde_notifications.sh from flatpak..."
    @flatpak run --command=cat {{flatpak_id}} /app/share/cosmic-connect/disable_kde_notifications.sh > /tmp/disable_kde_notifications.sh
    @chmod +x /tmp/disable_kde_notifications.sh
    @/tmp/disable_kde_notifications.sh
    @rm /tmp/disable_kde_notifications.sh
    @echo "✓ KDE notifications disabled"

# Enable KDE notifications (for flatpak)
flatpak-enable-kde-notifications:
    @echo "Extracting and running show_kde_notifications.sh from flatpak..."
    @flatpak run --command=cat {{flatpak_id}} /app/share/cosmic-connect/show_kde_notifications.sh > /tmp/show_kde_notifications.sh
    @chmod +x /tmp/show_kde_notifications.sh
    @/tmp/show_kde_notifications.sh
    @rm /tmp/show_kde_notifications.sh
    @echo "✓ KDE notifications enabled"

# Disable KDE notifications
disable-kde-notifications:
    ./scripts/disable_kde_notifications.sh

# Enable KDE notifications
enable-kde-notifications:
    ./scripts/show_kde_notifications.sh

# View logs from the applet
logs:
    journalctl --user -f -u cosmic-panel | grep -i kdeconnect

# View flatpak logs
logs-flatpak:
    flatpak run --command=sh {{flatpak_id}} -c "journalctl --user -f | grep -i kdeconnect"

# View system logs
logs-system:
    journalctl -f | grep -i kdeconnect

# Monitor D-Bus traffic for KDE Connect
monitor-dbus:
    dbus-monitor --session "type='signal',interface='org.kde.kdeconnect.device'"

# ================================
# Development Workflows
# ================================

# Development setup: install deps, build, and install (run with: sudo just dev-setup)
dev-setup: install-deps
    cargo build --release
    just install
    @echo "✓ Development environment ready!"

# Development setup with flatpak (run with: sudo just dev-setup-flatpak for deps)
dev-setup-flatpak: install-deps install-flatpak-runtimes flatpak-build-full
    @echo "✓ Development environment ready (flatpak)!"

# Rebuild and quick install (for rapid iteration, run with: sudo just rebuild)
rebuild:
    cargo build --release
    just install-quick

# Clean, rebuild, and install everything (run with: sudo just fresh)
fresh: clean
    cargo build --release
    just install
    @echo "✓ Fresh installation complete!"

# Clean, rebuild flatpak
fresh-flatpak: clean-flatpak flatpak-build
    @echo "✓ Fresh flatpak build complete!"

# Create a release (bundle + standard build) - use 'sudo just install' after to install binaries
release: build-release flatpak-bundle
    @echo "✓ Release ready!"
    @echo "  - Standard binaries in target/release/"
    @echo "  - Flatpak bundle: {{flatpak_id}}.flatpak"
