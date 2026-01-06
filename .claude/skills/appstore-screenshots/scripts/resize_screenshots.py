#!/usr/bin/env python3
"""
Resize screenshots to App Store required dimensions.
"""

import os
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("Error: Pillow not installed. Run: pip install Pillow")
    sys.exit(1)

# App Store screenshot sizes
SIZES = {
    "6.7": (1290, 2796),   # iPhone 15 Pro Max, 14 Pro Max
    "6.5": (1284, 2778),   # iPhone 14 Plus, 13 Pro Max, 12 Pro Max
    "6.5-alt": (1242, 2688),  # iPhone 11 Pro Max, XS Max
    "5.5": (1242, 2208),   # iPhone 8 Plus, 7 Plus, 6s Plus
    "ipad-12.9": (2048, 2732),  # iPad Pro 12.9"
    "ipad-11": (1668, 2388),    # iPad Pro 11"
}

def resize_screenshots(input_dir, output_dir, target_size="6.5"):
    """Resize all images in input_dir to target size."""

    if target_size not in SIZES:
        print(f"Error: Unknown size '{target_size}'")
        print(f"Available: {', '.join(SIZES.keys())}")
        sys.exit(1)

    width, height = SIZES[target_size]
    os.makedirs(output_dir, exist_ok=True)

    count = 0
    for filename in sorted(os.listdir(input_dir)):
        if not filename.lower().endswith(('.png', '.jpg', '.jpeg')):
            continue

        input_path = os.path.join(input_dir, filename)
        output_filename = Path(filename).stem + '.png'
        output_path = os.path.join(output_dir, output_filename)

        img = Image.open(input_path)
        resized = img.resize((width, height), Image.LANCZOS)

        if resized.mode == 'RGBA':
            resized = resized.convert('RGB')

        resized.save(output_path, 'PNG')
        print(f"  {filename} -> {output_filename} ({width}x{height})")
        count += 1

    return count

def main():
    if len(sys.argv) < 2:
        print("Usage: python resize_screenshots.py <input_dir> [output_dir] [size]")
        print(f"\nSizes: {', '.join(SIZES.keys())}")
        print("\nExample:")
        print("  python resize_screenshots.py ./screenshots ./appstore 6.5")
        sys.exit(1)

    input_dir = sys.argv[1]
    output_dir = sys.argv[2] if len(sys.argv) > 2 else os.path.join(input_dir, "appstore")
    target_size = sys.argv[3] if len(sys.argv) > 3 else "6.5"

    print(f"Input: {input_dir}")
    print(f"Output: {output_dir}")
    print(f"Target: {target_size} ({SIZES[target_size][0]}x{SIZES[target_size][1]})")
    print()

    count = resize_screenshots(input_dir, output_dir, target_size)
    print(f"\n✓ Resized {count} screenshots")

if __name__ == "__main__":
    main()
