# LogLens v0.54.3 — Cursor Values

Cursor Values now uses a compact selection title and a smaller source line above three equal-width A, B and delta columns. Long names and numbers are truncated instead of widening the panel; hover to see full text. Per-waveform cursor lists scroll inside their lane and stay aligned with the graph. Partially scrolled rows are clipped in place rather than jumping their labels to the top of the panel.

The source ZIP retains all v0.54.1 session restoration and v0.54.2 graph-header/scrolling fixes. Build with Cargo or the supplied platform scripts.

Validation: the headless egui layout test passes for long source names, large values and eight cursors at the minimum 280-pixel panel width. Native GUI appearance has not been visually verified. The pre-existing unrelated container-decoder test failure is unchanged.
