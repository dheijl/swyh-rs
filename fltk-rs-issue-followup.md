Update: the changes described above are now tested on Windows with MSVC 14.52, both locally and on GitHub's `windows-latest` runner. The build succeeds, and `fltk.patch` applies without errors.

They're in two forks, in case they're useful:

- cfltk, `fltk1.4` branch: [dheijl/cfltk@06dc346](https://github.com/dheijl/cfltk/commit/06dc346d36b920c406ca7e25956ca7ccfd29ccf1). This moves the `fltk` submodule to `a4b9c29` and refreshes `fltk.patch`.
- fltk-rs: [dheijl/fltk-rs@1e76666](https://github.com/dheijl/fltk-rs/commit/1e766665af94d730f0f6d673bfb52e46d2a5d11b). This points `fltk-sys/cfltk` at that commit. It also changes the URL in `.gitmodules` to my fork, so it isn't meant to be merged as-is. Upstream only needs the submodule pointer to move to the corresponding MoAlyousef/cfltk commit.

For now we build swyh-rs with `[patch.crates-io] fltk-sys = { git = "https://github.com/dheijl/fltk-rs" }`, and will switch back to the crates.io release once this is fixed upstream. I'm happy to open a PR against MoAlyousef/cfltk `fltk1.4` if that helps.
