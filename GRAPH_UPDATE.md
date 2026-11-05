# LogLens v0.54.2

Graph headers use a consistent protocol · channel · frame · selection format. Bit selections now retain the same configured channel name/ID as byte selections. Configured signal headers include their channel and frame; wildcard signals explicitly say All channels. Cursor readouts include the same source identity.

Vertical graph scroll offsets are now retained unconditionally, including scrollbar dragging outside the plot rectangle and momentum after leaving the plot. The vertical scrollbar stays visible and narrow panes no longer force a 420-pixel plot width over the scrollbar. Plain vertical wheel/trackpad movement scrolls lanes; Shift-wheel pans time and Ctrl/Cmd-wheel zooms time.

The archive retains the v0.54.1 per-log session restoration changes. Build with Cargo or the included platform scripts; this is a source ZIP, not a prebuilt application.

Validation: two new headless egui tests pass for channel headers and scrollbar/wheel interaction. Native mouse/trackpad hardware and Windows/Linux were not exercised. The pre-existing container-decoder test failure remains unrelated to this update.
