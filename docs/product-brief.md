# MSX product brief

MSX is a macOS VST3 meter for producers mixing toward a reference song. Its audio path passes the input through at unity with zero latency.

Configure contiguous frequency bands and capture a representative passage from the DAW. MSX saves the highest sustained level reached in each band as a fixed target for both the reference track and other songs. Keep the instance on a shared bus, copy it to another track, or use the host's plug-in presets to carry the reference into another project. As the mix plays, each band stays idle until it exceeds its target. Bring up an element until the band triggers, then pull it back until the bar clears.

Use six bands by default. The working surface has band-count controls, editable crossover frequencies, capture controls, an adjustable peak hold and an offset for aligning the entire target to a different gain stage. A band layout belongs to its captured target. Clearing the saved reference makes that layout editable again. Capturing a new reference on the same layout keeps the old one until the user accepts the new capture.

Show the live RMS level and the fixed trigger threshold on the same dBFS scale, with both values visible. Keep the live fill blue and use orange above the line when the band triggers. Hold a separate peak marker and overage reading so brief triggers remain visible. Let the user set the hold from 100 to 2,000 ms with a 500 ms default. Keep capture and comparison in the same restrained, instrument-like view. Use Tab for focus, Return or Space for buttons, and arrow keys on frequency, offset and hold controls. Save the layout and accepted targets in the host's project and preset state.

A 1 dB margin keeps small deviations idle. Compare passages with similar instrumentation and gain staging.
