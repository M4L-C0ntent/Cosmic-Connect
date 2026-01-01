#!/bin/bash
# scripts/install-deps.sh

echo "Installing missing dependencies..."

missing=""

while IFS= read -r pkg; do
    # Skip empty lines
    if [ -z "$pkg" ]; then
        continue
    fi
    
    # Skip comments
    if [[ "$pkg" =~ ^# ]]; then
        continue
    fi
    
    # Check if package is installed
    if ! dpkg -s "$pkg" >/dev/null 2>&1; then
        echo "Need to install: $pkg"
        missing="$missing $pkg"
    fi
done < build-deps.list

if [ -n "$missing" ]; then
    echo ""
    echo "Installing packages:$missing"
    apt install -y $missing
    echo "✓ Dependencies installed"
else
    echo "✓ All dependencies already installed"
fi