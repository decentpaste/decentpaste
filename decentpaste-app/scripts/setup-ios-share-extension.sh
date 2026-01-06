#!/bin/bash
#
# setup-ios-share-extension.sh
#
# Sets up the iOS Share Extension after `yarn tauri ios init`.
# This script is REQUIRED after every iOS project regeneration.
#
# Usage:
#   cd decentpaste-app
#   yarn tauri ios init
#   ./scripts/setup-ios-share-extension.sh
#   open src-tauri/gen/apple/decentpaste-app.xcodeproj
#
# Configuration (read from tauri.conf.json):
#   - version    → Used for MARKETING_VERSION and CURRENT_PROJECT_VERSION
#   - identifier → Used to derive:
#       - Extension bundle ID: <identifier>.ShareExtension
#       - App Group: group.<identifier>
#
# What this script does:
#   1. Copies ShareExtension source files from plugin to gen/apple/
#   2. Creates entitlements files with App Groups (using derived App Group ID)
#   3. Adds ShareExtension target to project.yml (using derived bundle IDs)
#   4. Runs xcodegen to regenerate the Xcode project
#   5. Restores files overwritten by xcodegen (Info.plist, entitlements)
#
# Prerequisites:
#   - xcodegen installed (brew install xcodegen)
#   - Tauri iOS project initialized (yarn tauri ios init)
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(dirname "$SCRIPT_DIR")"
GEN_APPLE="$APP_DIR/src-tauri/gen/apple"
PLUGIN_IOS="$APP_DIR/tauri-plugin-decentshare/ios"
SHARE_EXT_DIR="$GEN_APPLE/ShareExtension"

# Extract configuration from tauri.conf.json
TAURI_CONF="$APP_DIR/src-tauri/tauri.conf.json"
if [ -f "$TAURI_CONF" ]; then
    APP_VERSION=$(grep '"version"' "$TAURI_CONF" | head -1 | sed 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
    APP_IDENTIFIER=$(grep '"identifier"' "$TAURI_CONF" | head -1 | sed 's/.*"identifier"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
else
    APP_VERSION="1.0.0"
    APP_IDENTIFIER="com.example.app"
    echo -e "${YELLOW}Warning: Could not read from tauri.conf.json${NC}"
fi

# Derive related identifiers
EXTENSION_IDENTIFIER="${APP_IDENTIFIER}.ShareExtension"
APP_GROUP="group.${APP_IDENTIFIER}"

echo -e "${GREEN}=== iOS Share Extension Setup ===${NC}"
echo "App directory: $APP_DIR"
echo "App version: $APP_VERSION"
echo "App identifier: $APP_IDENTIFIER"
echo "Extension identifier: $EXTENSION_IDENTIFIER"
echo "App Group: $APP_GROUP"
echo ""

# Check prerequisites
if [ ! -d "$GEN_APPLE" ]; then
    echo -e "${RED}Error: $GEN_APPLE does not exist.${NC}"
    echo "Run 'yarn tauri ios init' first."
    exit 1
fi

if ! command -v xcodegen &> /dev/null; then
    echo -e "${RED}Error: xcodegen is not installed.${NC}"
    echo "Install with: brew install xcodegen"
    exit 1
fi

if [ ! -f "$PLUGIN_IOS/ShareExtension/ShareViewController.swift" ]; then
    echo -e "${RED}Error: ShareViewController.swift not found in plugin.${NC}"
    echo "Expected at: $PLUGIN_IOS/ShareExtension/ShareViewController.swift"
    exit 1
fi

# Step 1: Create ShareExtension directory and copy source files
echo -e "${YELLOW}Step 1:${NC} Copying ShareExtension source files from plugin..."
mkdir -p "$SHARE_EXT_DIR"
cp "$PLUGIN_IOS/ShareExtension/ShareViewController.swift" "$SHARE_EXT_DIR/"
cp "$PLUGIN_IOS/ShareExtension/Info.plist" "$SHARE_EXT_DIR/"
echo "  ✓ Copied ShareViewController.swift"
echo "  ✓ Copied Info.plist"

# Step 2: Create ShareExtension entitlements
echo -e "${YELLOW}Step 2:${NC} Creating ShareExtension entitlements..."
cat > "$SHARE_EXT_DIR/ShareExtension.entitlements" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.application-groups</key>
    <array>
        <string>$APP_GROUP</string>
    </array>
</dict>
</plist>
EOF
echo "  ✓ Created ShareExtension.entitlements"

# Step 3: Update main app entitlements with App Groups
echo -e "${YELLOW}Step 3:${NC} Updating main app entitlements..."
MAIN_ENTITLEMENTS="$GEN_APPLE/decentpaste-app_iOS/decentpaste-app_iOS.entitlements"
cat > "$MAIN_ENTITLEMENTS" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.application-groups</key>
    <array>
        <string>$APP_GROUP</string>
    </array>
</dict>
</plist>
EOF
echo "  ✓ Updated decentpaste-app_iOS.entitlements"

# Step 4: Add ShareExtension target to project.yml
echo -e "${YELLOW}Step 4:${NC} Adding ShareExtension target to project.yml..."
PROJECT_YML="$GEN_APPLE/project.yml"

if grep -q "ShareExtension:" "$PROJECT_YML"; then
    echo "  ⚠ ShareExtension target already exists in project.yml, skipping..."
else
    # Append ShareExtension target
    cat >> "$PROJECT_YML" << EOF

  ShareExtension:
    type: app-extension
    platform: iOS
    sources:
      - path: ShareExtension
        excludes:
          - "*.entitlements"
    info:
      path: ShareExtension/Info.plist
    entitlements:
      path: ShareExtension/ShareExtension.entitlements
    settings:
      base:
        PRODUCT_BUNDLE_IDENTIFIER: $EXTENSION_IDENTIFIER
        TARGETED_DEVICE_FAMILY: "1,2"
        SWIFT_VERSION: "5.0"
        MARKETING_VERSION: "$APP_VERSION"
        CURRENT_PROJECT_VERSION: "$APP_VERSION"
        CODE_SIGN_STYLE: Automatic
        GENERATE_INFOPLIST_FILE: NO
        SKIP_INSTALL: YES
        IPHONEOS_DEPLOYMENT_TARGET: "14.0"
EOF
    echo "  ✓ Added ShareExtension target"
fi

# Step 5: Add ShareExtension as dependency of main app
echo -e "${YELLOW}Step 5:${NC} Adding ShareExtension as embedded dependency..."
if grep -q "target: ShareExtension" "$PROJECT_YML"; then
    echo "  ⚠ ShareExtension dependency already exists, skipping..."
else
    # Use sed to add the dependency after "- sdk: WebKit.framework"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' '/- sdk: WebKit.framework/a\
      - target: ShareExtension\
        embed: true\
        codeSign: true
' "$PROJECT_YML"
    else
        sed -i '/- sdk: WebKit.framework/a\      - target: ShareExtension\n        embed: true\n        codeSign: true' "$PROJECT_YML"
    fi
    echo "  ✓ Added ShareExtension as embedded dependency"
fi

# Step 6: Regenerate Xcode project
echo -e "${YELLOW}Step 6:${NC} Regenerating Xcode project with xcodegen..."
cd "$GEN_APPLE"
xcodegen generate --quiet
echo "  ✓ Xcode project regenerated"

# Step 7: Restore files AFTER xcodegen (xcodegen overwrites them with empty/generic versions)
echo -e "${YELLOW}Step 7:${NC} Restoring files overwritten by xcodegen..."

# Restore Info.plist with NSExtension dictionary
cp "$PLUGIN_IOS/ShareExtension/Info.plist" "$SHARE_EXT_DIR/"
echo "  ✓ Restored ShareExtension Info.plist"

# Restore ShareExtension entitlements with App Groups
cat > "$SHARE_EXT_DIR/ShareExtension.entitlements" << ENTEOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.application-groups</key>
    <array>
        <string>$APP_GROUP</string>
    </array>
</dict>
</plist>
ENTEOF
echo "  ✓ Restored ShareExtension entitlements with App Groups"

# Restore main app entitlements with App Groups
cat > "$MAIN_ENTITLEMENTS" << ENTEOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.application-groups</key>
    <array>
        <string>$APP_GROUP</string>
    </array>
</dict>
</plist>
ENTEOF
echo "  ✓ Restored main app entitlements with App Groups"

echo ""
echo -e "${GREEN}=== Setup Complete ===${NC}"
echo ""
echo "Next steps:"
echo "  1. Open Xcode:"
echo "     open $GEN_APPLE/decentpaste-app.xcodeproj"
echo ""
echo "  2. Configure code signing for BOTH targets:"
echo "     - Select decentpaste-app_iOS → Signing & Capabilities → Set Team"
echo "     - Select ShareExtension → Signing & Capabilities → Set SAME Team"
echo ""
echo "  3. Build and run on physical device (Cmd+R)"
echo ""
echo -e "${YELLOW}Note:${NC} If App Group '$APP_GROUP' doesn't exist,"
echo "      create it in Apple Developer Portal first."
