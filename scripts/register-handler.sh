#!/bin/bash
# scripts/register-handler.sh

# If running as root via sudo, re-run as the actual user
if [ "$USER" = "root" ] && [ -n "$SUDO_USER" ]; then
    echo "Running as root, switching to user: $SUDO_USER"
    exec sudo -u "$SUDO_USER" "$0" "$@"
fi

echo "Registering kdeconnect:// URL handler..."

xdg-mime default io.github.M4LC0ntent.CosmicConnectSettings.desktop x-scheme-handler/kdeconnect

# Verify registration
HANDLER=$(xdg-mime query default x-scheme-handler/kdeconnect)
if [ "$HANDLER" = "io.github.M4LC0ntent.CosmicConnectSettings.desktop" ]; then
    echo "✓ Successfully registered as pairing notification handler"
else
    echo "⚠ Handler registration may have failed. Current: $HANDLER"
fi