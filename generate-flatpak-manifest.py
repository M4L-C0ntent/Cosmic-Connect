#!/usr/bin/env python3
# generate-flatpak-manifest.py
"""
Generates a complete flatpak manifest by merging the base manifest with cargo sources.
"""

import json
import sys
import os

def generate_manifest(base_manifest_path, cargo_sources_path, output_path):
    """Generate complete flatpak manifest with cargo sources."""
    
    # Read the base manifest
    with open(base_manifest_path, 'r') as f:
        content = f.read()
        # Skip the comment line if present
        if content.startswith('//'):
            content = '\n'.join(content.split('\n')[1:])
        manifest = json.loads(content)
    
    # Read cargo sources
    with open(cargo_sources_path, 'r') as f:
        cargo_sources = json.load(f)
    
    # Find the cosmic-connect-applet module and add cargo sources
    for module in manifest['modules']:
        if module['name'] == 'cosmic-connect-applet':
            # Get existing sources (should be the directory source)
            existing_sources = module.get('sources', [])
            
            # Keep only directory sources
            dir_sources = [s for s in existing_sources if s.get('type') == 'dir']
            
            # Merge: directory source + all cargo sources
            module['sources'] = dir_sources + cargo_sources
            
            print(f"✓ Added {len(cargo_sources)} cargo sources to manifest")
            break
    
    # Write the complete manifest (no comments - JSON doesn't support them)
    with open(output_path, 'w') as f:
        json.dump(manifest, f, indent=2)
    
    print(f"✓ Generated complete manifest: {output_path}")

if __name__ == '__main__':
    if len(sys.argv) != 4:
        print("Usage: generate-flatpak-manifest.py <base-manifest.json> <cargo-sources.json> <output.json>")
        sys.exit(1)
    
    base_manifest_path = sys.argv[1]
    cargo_sources_path = sys.argv[2]
    output_path = sys.argv[3]
    
    if not os.path.exists(base_manifest_path):
        print(f"Error: Base manifest not found: {base_manifest_path}")
        sys.exit(1)
    
    if not os.path.exists(cargo_sources_path):
        print(f"Error: Cargo sources not found: {cargo_sources_path}")
        print("Run: just flatpak-gen-sources")
        sys.exit(1)
    
    generate_manifest(base_manifest_path, cargo_sources_path, output_path)