# Design: Remove CLAUDE.md Managed Section When Last Skill Is Uninstalled

**Date:** 2026-03-12

## Problem

When all skreg skill packages are uninstalled, the `<!-- skreg:start --> ... <!-- skreg:end -->` block remains in `~/.claude/CLAUDE.md` with `- (none)` as the skill list. It should be removed entirely.

## Decision

**Modify `write_claude_md` in `linker.rs`** to branch on whether `entries` is empty:

- **Non-empty entries**: existing behavior — render and write/replace the managed section.
- **Empty entries**: if the file contains the skreg markers, strip the block (keep everything before `<!-- skreg:start -->` and everything after `<!-- skreg:end -->`). If no markers exist, do nothing. If the file doesn't exist, do nothing.

No changes to call sites in `install.rs` or `uninstall.rs` — both already call `write_claude_md` unconditionally after updating the linker state.

## Out of Scope

- Cleaning up surrounding blank lines (acceptable to leave).

## Tests

- New test: after uninstalling the last skill, CLAUDE.md contains no skreg markers.
- Existing tests remain unchanged.
