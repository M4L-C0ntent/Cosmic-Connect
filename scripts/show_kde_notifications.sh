#!/bin/bash

# scripts/show_kde_notifications.sh

# If running as root via sudo, re-run as the actual user
if [ "$USER" = "root" ] && [ -n "$SUDO_USER" ]; then
    echo "Running as root, switching to user: $SUDO_USER"
    exec sudo -u "$SUDO_USER" "$0" "$@"
fi

echo "=== Reverting KDE Connect Notification Configuration ==="
echo ""

echo "Step 1: Remove notification config files"
echo "-----------------------------------------"

if [ -f "$HOME/.config/kdeconnect.notifyrc" ]; then
    rm "$HOME/.config/kdeconnect.notifyrc"
    echo "âœ“ Removed kdeconnect.notifyrc"
else
    echo "  kdeconnect.notifyrc not found"
fi

if [ -f "$HOME/.config/kdeconnectrc" ]; then
    # Backup first
    cp "$HOME/.config/kdeconnectrc" "$HOME/.config/kdeconnectrc.backup"
    
    # Remove notification-related entries
    kwriteconfig5 --file kdeconnectrc --group "Notifications" --delete
    kwriteconfig5 --file kdeconnectrc --group "General" --key "ShowNotifications" --delete
    
    echo "âœ“ Cleaned up kdeconnectrc"
else
    echo "  kdeconnectrc not found"
fi

echo ""
echo "Step 2: Restart KDE Connect"
echo "---------------------------"

killall kdeconnectd 2>/dev/null
sleep 2

echo "âœ“ KDE Connect daemon killed"
echo "It will restart automatically when needed"
echo ""
echo "âœ“ Configuration reverted"
echo "KDE Connect notifications will now appear normally"