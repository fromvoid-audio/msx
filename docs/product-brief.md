# MXH product brief

MXH is a macOS VST3 meter for producers mixing toward a reference song. Its audio path passes the input through at unity with zero latency.

Configure contiguous frequency bands and capture a representative passage from the DAW. MXH learns the highest sustained level in each band. Keep the instance on a shared bus, copy it to another track, or use the host's plug-in presets to carry the reference into another project. As the mix plays, each band shows its signed distance from the target. Bring up an element until its band crosses the target, then pull it back to the edge of the neutral region.

Use six bands by default. The working surface has band-count controls, editable crossover frequencies, capture controls and an offset for aligning the entire target to a different gain stage. A band layout belongs to its captured target. Clearing the saved reference makes that layout editable again. Capturing a new reference on the same layout keeps the old one until the user accepts the new capture.

Make high, low and near-target states readable through bar direction, a signed dB reading and color. Keep capture and comparison in the same restrained, instrument-like view. Use Tab for focus, Return or Space for buttons, and arrow keys on frequency and offset controls. Save the layout and accepted targets in the host's project and preset state.

Initial design choices are fixed-level comparison, a ±1 dB neutral region, and strongest sustained levels rather than instantaneous sample peaks. The intended comparison is between similar musical sections and gain stages.
