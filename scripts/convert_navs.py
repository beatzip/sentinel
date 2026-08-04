#!/usr/bin/env python3
"""Script to convert binary .nav files to JSON format for the sentry project."""

import sys
from pathlib import Path

try:
    from awpy.nav import Nav
except ImportError:
    print("Error: awpy not installed. Install with: pip install awpy")
    sys.exit(1)

def convert_all_navs():
    """Convert all binary .nav files from awpy to JSON format."""
    # Source directory with binary nav files
    source_dir = Path.home() / ".awpy" / "nav"
    
    # Target directory in the project
    target_dir = Path(__file__).parent.parent / "crates" / "sentinel-map" / "assets" / "nav"
    
    # Ensure target directory exists
    target_dir.mkdir(parents=True, exist_ok=True)
    
    if not source_dir.exists():
        print(f"Source directory not found: {source_dir}")
        print("Download nav files with: python -c \"from awpy import get_nav; get_nav()\"")
        return
    
    # Find all .nav files
    nav_files = list(source_dir.glob("*.nav"))
    if not nav_files:
        print("No .nav files found in awpy directory")
        return
    
    converted = 0
    skipped = 0
    errors = 0
    
    for nav_file in nav_files:
        map_name = nav_file.stem
        json_file = target_dir / f"{map_name}.json"
        
        # Check if JSON already exists and is newer
        if json_file.exists() and json_file.stat().st_mtime >= nav_file.stat().st_mtime:
            print(f"⊘ Skipped (up to date): {map_name}")
            skipped += 1
            continue
        
        try:
            # Read binary nav file
            nav = Nav.from_path(nav_file)
            
            # Save as JSON
            nav.to_json(json_file)
            
            print(f"✓ Converted: {map_name} ({nav_file.stat().st_size / 1024:.0f}KB -> {json_file.stat().st_size / 1024:.0f}KB)")
            converted += 1
            
        except Exception as e:
            print(f"✗ Error converting {map_name}: {e}")
            errors += 1
    
    print(f"\n=== Conversion Summary ===")
    print(f"Converted: {converted}")
    print(f"Skipped (up to date): {skipped}")
    print(f"Errors: {errors}")
    print(f"Output directory: {target_dir}")

if __name__ == "__main__":
    convert_all_navs()