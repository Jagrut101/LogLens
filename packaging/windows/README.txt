LogLens v0.54.42 — Windows 64-bit

INSTALL
1. Right-click the ZIP and choose Extract All.
2. Open the extracted LogLens folder.
3. Double-click Install.cmd to install for your account and create Desktop
   and Start menu shortcuts. No administrator access is required.

Or run LogLens.exe directly for portable use. Keep the included DLL files
beside it. Rust and MinGW are not needed on the Windows PC.
Close the installed application before updating.
Installation location: %LOCALAPPDATA%\Programs\LogLens

NEW CONTROLS
- Full capture loading now streams decoded data against an estimated RAM
  budget instead of stopping at a fixed record count. Genuine memory limits
  still report partial loading. TC_OCM_CD_01.01.pcapng was verified with all
  44 channels and 2,922,727 normalized records across 31.676 seconds.
- Full Range restores the entire loaded view and clears time restrictions
  while keeping channel, frame and graph selections. New captures show the
  complete loaded span; saved workspace zoom still restores.
- New captures default to TECMP source time for analog/CM alignment.
  Capture-time analog graphs explain buffering delay and offer Use TECMP
  source time. Clock switching retains selections, names and filters;
  the view and cursors reset to the new timeline.
- Channel IDs use compact hexadecimal consistently: 0x1, 0x1234, 0x8011.
  No leading zero padding. Existing decimal/hex input and saved selections
  remain supported; trace copying and CSV/Excel exports use compact hex.
- Partial loads/retries keep the full channel selection, including channels
  without plotted graphs or records in the loaded prefix. Disabled project
  channels stay disabled. A shorter partial retry keeps the existing data.
- Genuine memory/record limits can still restrict later data; the partial
  warning is no longer shown merely because a file ends exactly at the cap.
- Export images follow Windows light/dark appearance automatically.
  The application appearance is a fallback if the OS theme is unavailable.
- The graph window has its own Settings button for theme, grid, colors,
  cursor modes, line width, scale, layout, detail and lane height.
- Options > Save graph image… renders a clean system-themed plot directly from data,
  with axes, labels and a cursor measurement table. No screenshot, window
  controls, side panels or unused screen space. All selected/filtered traces
  in the current time window are included, even below the scroll position.
- Activity beside Frame IDs filters Pattern, Constant, Counter, Signal,
  CRC / Checksum candidate, Analog and Other. Use Select none, then check
  one or more types. Clear filter restores all types.
- Double-click or right-click a graph title to rename it.
- Right-click a channel in the Graphics message list to rename that channel.
- Clear a name to restore its original label. Names are saved per workspace;
  decoding IDs remain unchanged.
- ETH Source IP and Destination IP menus support multiple checkboxes.
  Uncheck All before selecting only particular addresses.
- Click … [expand] on clipped Data cells to wrap the complete payload inline.
- The × icon shares the graph header row with its title and Drag handle.
- Reload selected channels retains edited names, filters, ranges and cursors.
  Click Fit separately to view the full newly loaded duration.
- Channel IDs are shown in hexadecimal (for example 0x8011). Frame IDs retain protocol formatting.

VERIFICATION
142 active macOS tests passed. The supplied capture reload regression also
passed using LogLens's saved capture copy. This Windows release was
cross-compiled for x86_64-pc-windows-gnu, with PE format and DLL dependencies
checked. Windows startup and installation have not been run on a Windows PC.
