---
name: bump-version
description: Bump version numbers across all DecentPaste config files (package.json, Cargo.toml, tauri.conf.json, downloads.json). Use for version updates without building.
allowed-tools: Read, Edit, AskUserQuestion
---

# Bump Version

Updates version (x.x.x format) in all config files.

## Files to Update

From project root:
- `decentpaste-app/package.json` → `"version": "x.x.x"`
- `decentpaste-app/src-tauri/Cargo.toml` → `version = "x.x.x"`
- `decentpaste-app/src-tauri/tauri.conf.json` → `"version": "x.x.x"`
- `website/downloads.json` → `version`, `tag` (with v prefix), and ALL asset URLs

## Steps

1. Read current version from `tauri.conf.json`
2. Ask user for new version (validate: `^\d+\.\d+\.\d+$`)
3. Edit all 4 files (use `replace_all: true` for downloads.json)
4. Confirm: list updated files, remind to commit
