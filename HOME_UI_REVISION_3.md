# LogLens Home UI — Revision 3: native visual parity pass

**Scope:** `src/app.rs` home screen only, based on Home UI Revision 2. Package version remains 0.54.42. This is a source-only UI revision; it is not a validated macOS binary.

## Why the previous build did not match the HTML preview

Revision 2 corrected viewport usage and rearranged the cards, but still used egui's default small buttons, tight spacing, plain grouped frames, a compact toolbar, equal-width columns, and inconsistent card heights. A floating Settings window in the screenshot also obscured the page. The HTML preview does not implement the native app's Settings window.

## Native changes

- Header: dark/light surface colors, ~70px header treatment, custom waveform brand icon, larger LogLens heading, Home label, theme switch, Settings, Tool Log.
- Hero: enlarged, two-line headline, 14px description, three always-visible actions (unavailable actions disabled), teal primary button, status pill, bigger margins, rounded card, accent border.
- Page: HTML reference dark/light color palette (#0b1218 background in dark mode; #151e27 cards), responsive 28px outer margins, larger control padding, and consistent 14px/17px body/card typography.
- Overview: adaptive 44%/56% Configuration Library / Recent Captures columns on wide layouts, equal-height cards filling remaining height, stacking when <980 logical pixels.
- Configuration Library: five equal-sized import buttons at large widths, responsive wrapping at smaller widths.
- Recent Captures: expanded search field, larger row text/buttons, full-width panel, scrollable list; all existing Open/Remove/Refresh handlers retained.
- Channel selection: rounded, accented panel, with original TECMP/capture time toggle and channel filtering/selection retained.
- Footer: readable workflow and status line; revision marker `Home UI revision 3` verifies which native binary was launched.

## Backend unchanged

No changes to decoding, indexing, capture reader, background loader, waveform processing, export code, or persistence format. The existing v0.54.42 release validation issues (including PCAPNG segmented export and platform packaging) remain open.

## Difference from HTML reference

The HTML reference demonstrates CSS layout, shadows and gradients and includes illustrative sessions. The native application is rendered with egui, so its control painting, font metrics and window chrome will not be pixel-identical to a browser. Unlike HTML, the native Settings panel is functional and floats over the right side when opened; close it to compare the underlying home screen.

## Build on Mac (Apple Silicon)

```bash
cd /path/to/LogLens_v0.54.42_Home_UI_Revision3
cargo check --locked
cargo test --locked
bash build_macos_app.sh
open dist/LogLens.app
```

Close older LogLens processes before launching. Verify `Home UI revision 3` in the bottom-right status bar. If it still displays Revision 2, the old bundle is being launched.

## Recommended on-device validation

1. Launch without a capture; compare with HTML preview at 1440x900 and 1920x1080.
2. Verify Header, Hero and both cards use available width, with visually matched card heights.
3. Resize to <980 logical px; verify stacking and no horizontal clipping.
4. Open PCAPNG, check channel picker and both timestamp choices; load a selection, return Home and Resume.
5. Test Recent Captures search, Open, Remove and Refresh, including long filenames and 12+ entries.
6. Test Configuration imports and dark/light themes, then Settings, Tool Log and reopened capture workflows.

**Validation here:** static source checks and ZIP integrity only. No Rust compiler/toolchain or macOS GUI available in this runtime; cargo check, cargo test and native rendering are unverified.
