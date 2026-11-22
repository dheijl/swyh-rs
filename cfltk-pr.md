Fixes the Windows source build of fltk-rs with MSVC 14.52 (VS 2026 18.12). See fltk-rs/fltk-rs#1706.

## Changes

**1. Bump `fltk` submodule: `88eedd3` → `a4b9c29` (current `branch-1.4` head)**

MSVC 14.52 treats `copysign` as an intrinsic, so FLTK's own definition in `FL/math.h` fails:

```text
fltk/src/FL/math.h(46): error C2169: 'copysign': intrinsic function, cannot be defined
```

Upstream FLTK fixed this in [fltk/fltk@a4b9c29](https://github.com/fltk/fltk/commit/a4b9c2975a2d67435300e3f96a02c751dc458a04), which skips the `rint`/`copysign` replacements on MSVC ≥ 1800. The other 8 commits in the bump are bug fixes only: a use-after-free in `Fl_Cairo_Graphics_Driver::clean_utf8()`, the copied SVG image crash, and `Fl_Terminal` redraw and doc fixes. None of them change the API. `src/Fl_win32.cxx` is identical in both commits.

**2. Refresh `fltk.patch`**

The patch no longer applied (`patch failed: src/Fl_win32.cxx:1200`). Upstream FLTK added `invalidate_gl_win()` and `child_window_cb()` before `WndProc`, so the context for `static bool left_window = true;` no longer matched. Because `git apply` applies all hunks or none, Windows source builds were skipping the whole patch, including the `~Fl_Win32_At_Exit` cleanup change.

The patch was regenerated from the FLTK tree. The changes it makes are the same. Only the hunk offsets, the context for the `left_window` declaration, and the index line differ.

## Testing

- `git apply --check fltk.patch` passes on `a4b9c29`.
- fltk-rs (`fltk-sys` 1.5.23, `use-ninja`) built with this commit as its `cfltk` submodule:
  - Windows, MSVC 14.52: builds, and the patch applies. Tested locally and on GitHub's `windows-latest` runner.
  - Linux: builds.
- Used in [swyh-rs](https://github.com/dheijl/swyh-rs) via `[patch.crates-io]`. The GUI builds on Windows.

After this is merged, fltk-rs only needs its `fltk-sys/cfltk` submodule moved to the merge commit.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
