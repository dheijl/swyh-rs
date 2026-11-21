**Title:** Windows build fails with MSVC 14.52 (VS 2026 18.12): `copysign` C2169 in FL/math.h, and fltk.patch no longer applies

**Description**

With the MSVC 14.52 toolset that ships with Visual Studio 2026 18.12, building `fltk-sys` 1.5.23 from source fails while compiling the bundled FLTK. Earlier VS 2026 toolsets are not affected: MSVC 14.51 builds fine. This now breaks CI on GitHub's `windows-latest` runner, and it will break local builds as soon as VS updates to 14.52:

```text
fltk/src/FL/math.h(46): error C2169: 'copysign': intrinsic function, cannot be defined
```

The error is raised while compiling `Fl_Chart.cxx` (built with the `use-ninja` feature).

**Environment**

- fltk / fltk-sys 1.5.23, features `["use-ninja"]`
- GitHub Actions `windows-latest` (Visual Studio 18, MSVC 14.52.37025): fails
- GitHub Actions `windows-2022` (VS 2022, MSVC 14.4x): builds fine
- Local VS 2026, MSVC 14.51 (cl 19.51.36263): builds fine

**Problems**

1. **`copysign` C2169 (fatal).** `FL/math.h` defines its own `rint` and `copysign` for every non-MinGW Windows build. MSVC 14.52 treats `copysign` as an intrinsic and rejects the redefinition. FLTK fixed this upstream in [fltk/fltk@afdb764](https://github.com/fltk/fltk/commit/afdb76446778fd2dab89820948e2c67f8ee8d3ba) (2026-10-23, on both `master` and `branch-1.4`):

   ```diff
   -#  if (defined(_WIN32) || defined(CRAY)) && !defined(__MINGW32__)
   +#  if (defined(_WIN32) || defined(CRAY)) && !defined(__MINGW32__) && (!defined(_MSC_VER) || _MSC_VER < 1800)
   ```

   That line is the only difference between `FL/math.h` in fltk-sys 1.5.23 and upstream `master`. Updating the bundled FLTK to a revision that includes this commit, or cherry-picking it, should be enough.

2. **`fltk.patch` no longer applies.** The build log shows:

   ```text
   error: patch failed: src/Fl_win32.cxx:1200
   error: src/Fl_win32.cxx: patch does not apply
   ```

   The build continues, so this isn't fatal, but the Windows patch is silently left out. The patch probably needs to be rebased onto the current FLTK source.

**Related**

PR #1705 tried to fix both problems but was closed without merging. This issue tracks the underlying problem.

**Workaround**

Pin the CI runner to `windows-2022`, or build with an MSVC toolset older than 14.52 (14.4x or 14.51).
