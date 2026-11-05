# eframe 0.33.3 native immediate viewport patch

Source: the locally cached crates.io eframe 0.33.3 release, retaining upstream source and licenses.

In src/native/glow_integration.rs, render_immediate_viewport now handles pending Screenshot/Cut/Copy/Paste actions after painting and before swapping buffers, matching the existing root/deferred viewport path. Screenshots are returned to that viewport's egui input queue with the original user data, and a repaint is requested. The upstream immediate renderer previously never drained actions_requested, so the graph window's screenshot command could not return an image.

The Cargo patch ensures both Mac and Windows builds use this source. No global Cargo registry files are modified.
