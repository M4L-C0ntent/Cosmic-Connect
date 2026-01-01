#!/bin/bash

# Add dep check to check if package dependencies are installed and install if not.
while read pkg; do
    [[ "$pkg" =~ ^#.*$ ]] && continue
    dpkg -s "$pkg" >/dev/null 2>&1 || missing_pkgs+="$pkg "
done < package-deps.list
[[ -n "$missing_pkgs" ]] && sudo apt install -y $missing_pkgs   



echo ""
echo "âœ“ Installation complete!"

exit 0