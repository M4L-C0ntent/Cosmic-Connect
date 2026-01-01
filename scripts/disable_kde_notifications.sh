#!/bin/bash

# scripts/disable_kde_notifications.sh

# If running as root via sudo, re-run as the actual user
if [ "$USER" = "root" ] && [ -n "$SUDO_USER" ]; then
    echo "Running as root, switching to user: $SUDO_USER"
    exec sudo -u "$SUDO_USER" "$0" "$@"
fi

echo "=== Disabling KDE Connect Notifications via KDE Config ==="
echo ""

# Check if kwriteconfig5 is available
if ! command -v kwriteconfig5 &> /dev/null; then
    echo "âœ— kwriteconfig5 not found"
    echo "Installing kwriteconfig5..."
    
    # Try to install it
    if command -v apt &> /dev/null; then
        sudo apt install -y kde-cli-tools
    elif command -v dnf &> /dev/null; then
        sudo dnf install -y kde-cli-tools
    elif command -v pacman &> /dev/null; then
        sudo pacman -S --noconfirm kde-cli-tools
    else
        echo "âœ— Cannot install kwriteconfig5 automatically"
        echo "Please install 'kde-cli-tools' package manually"
        exit 1
    fi
fi

echo "âœ“ kwriteconfig5 is available"
echo ""

echo "Step 1: Disable KDE Connect notification events"
echo "------------------------------------------------"

# KDE Connect's notification events are defined in its notifyrc file
# We need to disable the pairing notification event

KDECONNECT_NOTIFYRC="$HOME/.config/kdeconnect.notifyrc"

echo "Configuration file: $KDECONNECT_NOTIFYRC"
echo ""

# Disable the pairing request notification
echo "Disabling 'pairRequest' notification..."
kwriteconfig5 --file kdeconnect.notifyrc --group "Event/pairRequest" --key "Action" ""
kwriteconfig5 --file kdeconnect.notifyrc --group "Event/pairRequest" --key "Execute" ""
kwriteconfig5 --file kdeconnect.notifyrc --group "Event/pairRequest" --key "Sound" ""
kwriteconfig5 --file kdeconnect.notifyrc --group "Event/pairRequest" --key "Popup" "false"

echo "âœ“ Disabled pairRequest notification"
echo ""

# Also try disabling other notification events that might be related
echo "Disabling other KDE Connect notification events..."

EVENTS=(
    "pairingRequest"
    "pairingRequestReceived"
    "notification"
    "transferReceived"
    "transferComplete"
)

for event in "${EVENTS[@]}"; do
    kwriteconfig5 --file kdeconnect.notifyrc --group "Event/$event" --key "Action" ""
    kwriteconfig5 --file kdeconnect.notifyrc --group "Event/$event" --key "Popup" "false"
    echo "  âœ“ Disabled: $event"
done

echo ""
echo "Step 2: Check current configuration"
echo "------------------------------------"

if [ -f "$KDECONNECT_NOTIFYRC" ]; then
    echo "Current kdeconnect.notifyrc contents:"
    cat "$KDECONNECT_NOTIFYRC"
else
    echo "âš  Configuration file not created yet"
    echo "It will be created when KDE Connect is restarted"
fi

echo ""
echo "Step 3: Disable notifications in KDE Connect daemon config"
echo "----------------------------------------------------------"

# Also try disabling in the main KDE Connect config
KDECONNECT_CONFIG="$HOME/.config/kdeconnectrc"

echo "Configuration file: $KDECONNECT_CONFIG"
echo ""

# Create or modify the config
kwriteconfig5 --file kdeconnectrc --group "Notifications" --key "Enabled" "false"
kwriteconfig5 --file kdeconnectrc --group "General" --key "ShowNotifications" "false"

echo "âœ“ Updated kdeconnectrc"
echo ""

# Also check device-specific configs
echo "Step 4: Disable notifications for connected devices"
echo "----------------------------------------------------"

KDECONNECT_DIR="$HOME/.config/kdeconnect"

if [ -d "$KDECONNECT_DIR" ]; then
    for device_dir in "$KDECONNECT_DIR"/*/; do
        if [ -d "$device_dir" ]; then
            device_id=$(basename "$device_dir")
            config_file="${device_dir}config"
            
            if [ -f "$config_file" ]; then
                echo "Device: $device_id"
                
                # Disable notification plugin for this device
                kwriteconfig5 --file "$config_file" --group "notifications" --key "enabled" "false"
                kwriteconfig5 --file "$config_file" --group "kdeconnect_notifications" --key "enabled" "false"
                
                echo "  âœ“ Disabled notifications"
            fi
        fi
    done
else
    echo "No device configs found yet"
fi

echo ""
echo "Step 5: Restart KDE Connect daemon"
echo "-----------------------------------"

echo "Killing kdeconnectd..."
killall kdeconnectd 2>/dev/null
sleep 2

echo "Starting kdeconnectd..."
# Trigger D-Bus activation
dbus-send --session --print-reply \
    --dest=org.kde.kdeconnect \
    /modules/kdeconnect \
    org.kde.kdeconnect.daemon.devices \
    boolean:false boolean:false > /dev/null 2>&1 &

sleep 3

if pgrep -f kdeconnectd > /dev/null; then
    echo "âœ“ kdeconnectd is running"
else
    echo "âš  Starting manually..."
    /usr/lib/x86_64-linux-gnu/libexec/kdeconnectd &
    sleep 2
fi

echo ""
echo "Step 6: Verify configuration"
echo "-----------------------------"

echo ""
echo "kdeconnect.notifyrc:"
if [ -f "$KDECONNECT_NOTIFYRC" ]; then
    grep -A 3 "Event/pair" "$KDECONNECT_NOTIFYRC" 2>/dev/null || echo "  No pairing events configured"
else
    echo "  File not found (will be created on first use)"
fi

echo ""
echo "kdeconnectrc:"
if [ -f "$KDECONNECT_CONFIG" ]; then
    grep -A 2 "Notification" "$KDECONNECT_CONFIG" 2>/dev/null || echo "  No notification settings"
else
    echo "  File not found"
fi

echo ""
echo "=== Configuration Complete ==="