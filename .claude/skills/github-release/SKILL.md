---
name: github-release
description: Create a GitHub release with auto-generated release notes from commits since the last tag. Uses conventional commits to categorize changes into Features, Bug Fixes, and maintenance sections.
allowed-tools: Read, Bash, AskUserQuestion
---

# GitHub Release

Creates a GitHub release with formatted release notes based on commits since the last tag.

## Prerequisites

- GitHub CLI (`gh`) must be authenticated
- Repository must have at least one existing tag

## Steps

### 1. Gather Version Info

Read current version from `decentpaste-app/src-tauri/tauri.conf.json` and use it as the release version.

### 2. Get Latest Tag and Commits

```bash
# Get latest tag
git tag --sort=-v:refname | head -1

# Get commits since latest tag (non-merge commits with body)
git log <latest-tag>..HEAD --pretty=format:"%h %s%n%b---" --no-merges
```

### 3. Categorize Commits

Parse commits using conventional commit prefixes:

| Prefix                                                     | Category                |
|------------------------------------------------------------|-------------------------|
| `feat:` or `feat(...):`                                    | Features & Improvements |
| `fix:` or `fix(...):`                                      | Bug Fixes               |
| `chore:`, `docs:`, `refactor:`, `perf:`, `style:`, `test:` | Under the Hood          |

For each commit, extract:
- The short description (first line after prefix)
- Any bullet points from the commit body

### 4. Generate Release Notes

Format release notes as:

```markdown
## What's New in vX.X.X

[Brief 1-2 sentence summary of the release theme]

### Features & Improvements

- **Feature name** - Description from commit

### Bug Fixes

- **Fix name** - Description from commit

### Under the Hood

- **Change name** - Description from commit

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

**Full Changelog**: https://github.com/decentpaste/decentpaste/compare/<previous-tag>...v<version>
```

### 5. Ask for Confirmation

Show the user:
1. Version to be released
2. Previous tag being compared against
3. Number of commits included
4. Generated release notes preview

Ask: "Create this release?" with options Yes/No

### 6. Create Release

```bash
gh release create v<version> --title "DecentPaste v<version>" --notes "<generated-notes>"
```

Use a heredoc for the notes to preserve formatting:
```bash
gh release create vX.X.X --title "DecentPaste vX.X.X" --notes "$(cat <<'EOF'
<release notes here>
EOF
)"
```

### 7. Report Success

Show:
- Release URL
- Reminder to upload build artifacts with `gh release upload v<version> <file>`

## Notes

- If no commits match conventional format, include them in "Under the Hood"
- Skip merge commits (they clutter release notes)
- Omit empty sections from the release notes
- Version commits (`chore(release): bump version`) can be summarized or omitted
