I looked into this a bit further. A fix needs a change in cfltk first, then a submodule bump here.

**1. cfltk (`fltk1.4` branch): bump the `fltk` submodule**

cfltk's `fltk1.4` branch pins FLTK at `88eedd3` (2026-04-30). The current `branch-1.4` head is [`a4b9c29`](https://github.com/fltk/fltk/commit/a4b9c2975a2d67435300e3f96a02c751dc458a04), which is the branch-1.4 copy of the `copysign` / `rint` fix in `FL/math.h`. Moving the submodule to it fixes the C2169 error with no local edits to the FLTK sources. `src/Fl_win32.cxx` is identical between the two commits, so that file isn't affected by the bump.

**2. cfltk: refresh `fltk.patch`**

The patch failure is older than MSVC 14.52. `fltk.patch` doesn't apply to the FLTK source bundled in fltk-sys 1.5.23 either, on any compiler. Only one of its four hunks is stale. Upstream FLTK added `invalidate_gl_win()` / `child_window_cb()` between `fl_restore_pen()` and `WndProc`, so the context for `static bool left_window = true;` no longer matches.

Because `git apply` applies all hunks or none, Windows source builds currently skip **all** of the patch, including the hunks that still apply:

- the disabled cleanup in `~Fl_Win32_At_Exit` (the "security patch" mentioned in the build warning)
- the `FL_ENTER` handling in `WM_MOUSEMOVE`

Below is a regenerated patch with the same changes, where the `left_window` declaration moves to directly before `WndProc`. `git apply --check` passes against both `88eedd3` and `a4b9c29`.

<details>
<summary>Regenerated fltk.patch</summary>

```diff
diff --git a/src/Fl_win32.cxx b/src/Fl_win32.cxx
--- a/src/Fl_win32.cxx
+++ b/src/Fl_win32.cxx
@@ -572,16 +572,16 @@ class Fl_Win32_At_Exit {
 public:
   Fl_Win32_At_Exit() {}
   ~Fl_Win32_At_Exit() {
-    fl_free_fonts(); // do some Windows cleanup
-    fl_cleanup_pens();
-    OleUninitialize();
-    if (fl_graphics_driver) fl_brush_action(1);
-    fl_cleanup_dc_list();
-    // This is actually too late in the cleanup process to remove the
-    // clipboard notifications, but we have no earlier hook so we try
-    // to work around it anyway.
-    if (clipboard_wnd != NULL)
-      fl_clipboard_notify_untarget(clipboard_wnd);
+    // fl_free_fonts(); // do some Windows cleanup
+    // fl_cleanup_pens();
+    // OleUninitialize();
+    // if (fl_graphics_driver) fl_brush_action(1);
+    // fl_cleanup_dc_list();
+    // // This is actually too late in the cleanup process to remove the
+    // // clipboard notifications, but we have no earlier hook so we try
+    // // to work around it anyway.
+    // if (clipboard_wnd != NULL)
+    //   fl_clipboard_notify_untarget(clipboard_wnd);
 #if USE_GDIPLUS
     Fl_GDIplus_Graphics_Driver::shutdown();
 #endif
@@ -1223,6 +1223,8 @@ static BOOL CALLBACK child_window_cb(HWND child_xid, LPARAM data) {
 }
 
 
+static bool left_window = true;
+
 static LRESULT CALLBACK WndProc(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam) {
 
   // Copy the message to fl_msg so add_handler code can see it.
@@ -1402,6 +1404,14 @@ static LRESULT CALLBACK WndProc(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lPar
       }
 
       case WM_MOUSEMOVE:
+        if (left_window) {
+          Fl_Window *tw = window;
+          while (tw->parent()) // find top level window
+           tw = tw->window();
+          Fl::handle(FL_ENTER, tw);
+          left_window = false;
+          return 0;
+        }
 #ifdef USE_TRACK_MOUSE
         if (track_mouse_win != window) {
           TRACKMOUSEEVENT tme;
@@ -1421,6 +1431,7 @@ static LRESULT CALLBACK WndProc(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lPar
           while (tw->parent()) // find top level window
             tw = tw->window();
           Fl::belowmouse(0);
+          left_window = true;
           Fl::handle(FL_LEAVE, tw);
         }
         track_mouse_win = 0; // force TrackMouseEvent() restart
```

</details>

**3. fltk-rs: bump `fltk-sys/cfltk` to the resulting cfltk commit**

**Suggestion:** in `fltk-sys/build/source.rs`, the result of `git apply ../fltk.patch` is discarded with `.status().ok()`, which is why this went unnoticed. Emitting a `cargo:warning` when the exit status isn't success would make the next drift visible.

I've only checked that the patch applies. I haven't built or run it on Windows.
