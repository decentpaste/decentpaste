---
name: github-release
description: Create a GitHub release with auto-generated release notes from commits since the last tag. Uses conventional commits to categorize changes into Features, Bug Fixes, and maintenance sections.
---

# GitHub Release

Create GitHub release with formatted notes from commits since last tag.

**Prereqs**: `gh` authenticated, existing tags in repo

## Workflow

1. **Get version** from `decentpaste-app/src-tauri/tauri.conf.json`

2. **Get commits** since latest tag (run as separate commands):
   ```bash
   # First: get latest tag
   git tag --sort=-v:refname | head -1
   ```
   ```bash
   # Then: get commits since that tag (replace <TAG> with result above)
   git log <TAG>..HEAD --pretty=format:"%h %s%n%b---" --no-merges
   ```

3. **Categorize** by conventional commit prefix:

   | Prefix | Section |
   |--------|---------|
   | `feat:` | Features & Improvements |
   | `fix:` | Bug Fixes |
   | `chore:`, `docs:`, `refactor:`, `perf:`, `style:`, `test:` | Under the Hood |

4. **Generate notes** (omit empty sections):
   ```markdown
   ## What's New in vX.X.X

   [1-2 sentence summary]

   ### Features & Improvements
   - **Feature** - Description

   ### Bug Fixes
   - **Fix** - Description

   ### Under the Hood
   - **Change** - Description

   ### Downloads

   | Platform | Download |
   |----------|----------|
   | Windows | `DecentPaste_X.X.X_x64-setup.exe` |
   | macOS (Intel) | `DecentPaste_X.X.X_x64.dmg` |
   | macOS (Apple Silicon) | `DecentPaste_X.X.X_aarch64.dmg` |
   | Linux (Debian/Ubuntu) | `DecentPaste_X.X.X_amd64.deb` |
   | Linux (AppImage) | `DecentPaste_X.X.X_amd64.AppImage` |
   | Android | `DecentPaste_X.X.X.apk` |

   ---
   **Full Changelog**: https://github.com/decentpaste/decentpaste/compare/<prev-tag>...v<version>
   ```

5. **Confirm** with user: show version, tag, commit count, notes preview

6. **Create release**:
   ```bash
   gh release create vX.X.X --title "DecentPaste vX.X.X" --notes "$(cat <<'EOF'
   <notes>
   EOF
   )"
   ```

7. **Done**: Show release URL, remind to upload artifacts with `gh release upload`
