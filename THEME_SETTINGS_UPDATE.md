# LogLens v0.54.6 — Theme Settings

## Behavior
- Settings now contains an **Appearance** section with a **Theme** drop-down.
- Available themes: **Dark** and **Light**.
- Default: **Dark**.
- Theme changes are applied immediately.

## Persistence
The preference is application-wide rather than log/session-specific. On Windows it is stored at:

```text
%LOCALAPPDATA%\LogLens\settings.json
```

Example:

```json
{
  "theme": "Dark"
}
```

If the file does not exist, LogLens uses Dark. If the preference file cannot be loaded, LogLens falls back to Dark and records a warning in diagnostics.

## Theme-aware graphics
Waveform identity colors keep the existing Dark palette. Light theme uses a darker high-contrast palette, and A/B cursor colors use the existing theme-aware cursor colors so measurements remain readable on a light graph background.

## Scope
No capture decoding, analysis logic, session workflow, or diagnostic behavior was intentionally changed.
