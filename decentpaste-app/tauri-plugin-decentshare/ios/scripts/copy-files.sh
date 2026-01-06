#!/bin/bash
#
# Copy iOS Share Extension source files for easy access in Xcode.
#
# This script does NOT modify the Xcode project - you still need to
# add the files manually in Xcode. See README.md for step-by-step instructions.
#
# Usage:
#   cd tauri-plugin-decentshare/ios/scripts
#   ./copy-files.sh
#
# Or via yarn:
#   yarn ios:copy-files
#

set -e

# Get script directory and project paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_IOS_DIR="$(dirname "$SCRIPT_DIR")"
PLUGIN_DIR="$(dirname "$PLUGIN_IOS_DIR")"
APP_DIR="$(dirname "$PLUGIN_DIR")"
PROJECT_ROOT="$(dirname "$APP_DIR")"
APPLE_DIR="$APP_DIR/src-tauri/gen/apple"
DEST_DIR="$APPLE_DIR/ShareExtension-source"

echo "=============================================="
echo "  DecentPaste iOS Share Extension File Copy"
echo "=============================================="
echo ""
echo "Source directories:"
echo "  Plugin Swift:     $PLUGIN_IOS_DIR/Sources/"
echo "  ShareExtension:   $PLUGIN_IOS_DIR/ShareExtension/"
echo ""
echo "Destination:        $DEST_DIR"
echo ""

# Check if source files exist
if [ ! -f "$PLUGIN_IOS_DIR/ShareExtension/ShareViewController.swift" ]; then
    echo "ERROR: Source files not found!"
    echo "Expected: $PLUGIN_IOS_DIR/ShareExtension/ShareViewController.swift"
    echo ""
    echo "Make sure you're running this script from the correct location."
    exit 1
fi

# Check if gen/apple exists
if [ ! -d "$APPLE_DIR" ]; then
    echo "WARNING: iOS project not initialized!"
    echo "Expected: $APPLE_DIR"
    echo ""
    echo "Run 'yarn tauri ios init' first to generate the iOS project."
    echo ""
    echo "Creating destination directory anyway..."
fi

# Create destination directory
mkdir -p "$DEST_DIR"
mkdir -p "$DEST_DIR/Plugin"

# Copy ShareExtension files
echo "Copying ShareExtension files..."
cp "$PLUGIN_IOS_DIR/ShareExtension/ShareViewController.swift" "$DEST_DIR/"
cp "$PLUGIN_IOS_DIR/ShareExtension/Info.plist" "$DEST_DIR/"
echo "  - ShareViewController.swift"
echo "  - Info.plist"

# Copy plugin files
echo ""
echo "Copying Plugin files..."
cp "$PLUGIN_IOS_DIR/Sources/DecentsharePlugin.swift" "$DEST_DIR/Plugin/"
echo "  - DecentsharePlugin.swift"

echo ""
echo "=============================================="
echo "  Files copied successfully!"
echo "=============================================="
echo ""
echo "Files are now at:"
echo "  $DEST_DIR"
echo ""
echo "NEXT STEPS:"
echo ""
echo "1. Open Xcode:"
echo "   open $APPLE_DIR/decentpaste-app.xcodeproj"
echo ""
echo "2. Follow the iOS setup guide in README.md:"
echo "   - Add ShareExtension target"
echo "   - Replace generated files with our versions"
echo "   - Configure App Groups for both targets"
echo "   - Configure signing"
echo ""
echo "See: tauri-plugin-decentshare/README.md for detailed instructions"
echo ""
