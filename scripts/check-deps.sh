#!/bin/bash
# scripts/check-deps.sh

echo "Checking system dependencies..."

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
        echo "Missing: $pkg"
    fi
done < build-deps.list

echo "✓ Dependency check complete"