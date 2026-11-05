# IMPORTANT: Build the Rust source to see the new native home page

This ZIP contains updated **source code**, not a newly compiled application.
The HTML page `LogLens_Home_Page_Preview.html` is an independent design demonstration. It does not modify an installed LogLens executable.

## macOS (Apple silicon)

1. Quit the old LogLens app completely.
2. Open Terminal inside this extracted folder.
3. Run `cargo check --locked` to check the source, then `bash build_macos_app.sh` to build a native application.
4. Run `open dist/LogLens.app` **from this extracted folder**. Do not launch an old copy from /Applications.
5. Look for `Home UI revision 2` in the lower-right of the home screen. If absent, the running app is not this source revision.

Optionally install after confirming the updated build: `cp -R dist/LogLens.app /Applications/` (remove/rename old target first to avoid stale bundle contents).

## Windows (MinGW)

1. Open a terminal in this source folder with Rust and MinGW available.
2. Run `cargo check --locked`, then `build_windows_mingw.bat`.
3. Run the newly built `dist\LogLens.exe`; do not use an older downloaded executable.
4. Ensure required MinGW runtime DLLs are accessible. The existing Windows packaging script is not yet standalone/runtime-bundled.

## UI layout changes

- No fixed 1,060 px content cap. The native home fills the available CentralPanel width and height.
- Start maximized while preserving restore/resize.
- Two adaptive columns for configuration and recent sessions on wide windows; cards stack below 980 logical px.
- Scrollable capture/channel lists size relative to the available window height.

## Verification status

Source and ZIP structure checked in a Linux environment. **No Rust compiler or macOS/Windows GUI runtime was available**, so the source changes are not compile-tested or validated as a native running executable. Existing functional issues from the v0.54.42 validation report remain open.
