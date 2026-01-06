#!/usr/bin/env python3
"""
Remove alpha channel from iOS app icons.
Apple App Store requires icons with no transparency.
"""

import os
import sys
import subprocess
from pathlib import Path

def find_icon_dir():
    """Find the iOS app icon directory."""
    # Default location for Tauri iOS projects
    candidates = [
        "decentpaste-app/src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset",
        "src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset",
    ]

    for candidate in candidates:
        if os.path.isdir(candidate):
            return candidate

    return None

def check_has_alpha(filepath):
    """Check if a PNG has alpha channel using sips."""
    try:
        result = subprocess.run(
            ["sips", "-g", "hasAlpha", filepath],
            capture_output=True, text=True
        )
        return "hasAlpha: yes" in result.stdout
    except Exception:
        return None

def remove_alpha_pil(icon_dir):
    """Remove alpha channel using PIL (Pillow)."""
    try:
        from PIL import Image
    except ImportError:
        print("Error: Pillow not installed. Run: pip install Pillow")
        sys.exit(1)

    fixed_count = 0
    skipped_count = 0

    for filename in sorted(os.listdir(icon_dir)):
        if not filename.endswith('.png'):
            continue

        filepath = os.path.join(icon_dir, filename)
        img = Image.open(filepath)

        if img.mode == 'RGBA':
            # Create white background and composite
            background = Image.new('RGB', img.size, (255, 255, 255))
            background.paste(img, mask=img.split()[3])
            background.save(filepath, 'PNG')
            print(f"  Fixed: {filename}")
            fixed_count += 1
        elif img.mode == 'P' and 'transparency' in img.info:
            # Handle palette images with transparency
            img = img.convert('RGBA')
            background = Image.new('RGB', img.size, (255, 255, 255))
            background.paste(img, mask=img.split()[3])
            background.save(filepath, 'PNG')
            print(f"  Fixed (palette): {filename}")
            fixed_count += 1
        else:
            print(f"  Skipped (no alpha): {filename}")
            skipped_count += 1

    return fixed_count, skipped_count

def verify_icons(icon_dir):
    """Verify all icons have no alpha channel."""
    print("\nVerifying icons...")
    all_good = True

    for filename in sorted(os.listdir(icon_dir)):
        if not filename.endswith('.png'):
            continue

        filepath = os.path.join(icon_dir, filename)
        has_alpha = check_has_alpha(filepath)

        if has_alpha:
            print(f"  FAIL: {filename} still has alpha!")
            all_good = False
        else:
            print(f"  OK: {filename}")

    return all_good

def main():
    # Find icon directory
    icon_dir = sys.argv[1] if len(sys.argv) > 1 else find_icon_dir()

    if not icon_dir or not os.path.isdir(icon_dir):
        print("Error: Could not find icon directory.")
        print("Usage: python remove_alpha.py [path/to/AppIcon.appiconset]")
        sys.exit(1)

    print(f"Icon directory: {icon_dir}")
    print(f"\nRemoving alpha channel from icons...")

    # Remove alpha
    fixed, skipped = remove_alpha_pil(icon_dir)

    print(f"\nSummary: {fixed} fixed, {skipped} already OK")

    # Verify
    if verify_icons(icon_dir):
        print("\n All icons verified - no alpha channels!")
        sys.exit(0)
    else:
        print("\n Some icons still have alpha - check above!")
        sys.exit(1)

if __name__ == "__main__":
    main()
