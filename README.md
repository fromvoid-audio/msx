# MXH

MXH is a macOS VST3 reference meter built with Toybox. It measures band levels while passing mono or stereo audio through at unity with zero latency.

1. Insert MXH on a bus that can receive your reference track and your mix.
2. Set 2–8 bands and adjust their crossovers. Click the frequency steppers, drag a frequency vertically, or focus it and use the arrow keys.
3. Click **Capture reference**, play a representative section, and click **Use reference**. Capture at least one second of audible material.
4. Replay the captured passage at its original DAW position with **Compare** set to **Timeline**. Unchanged playback stays idle, including changes in dynamics within the passage.
5. For a different song or a moved clip, switch **Compare** to **Peak**. This uses the highest smoothed RMS level captured in each band. Raise a mix element until you see an orange bar, then back it off until the bar clears. Bars show only levels more than 1 dB above target.
6. Use **Target offset** to align the whole reference with your gain staging. Save the session or a host plug-in preset to reuse the reference.

Timeline comparison uses the DAW playhead and the sample rate used for capture. Positions outside the capture stay idle. Existing references open in Peak mode. Capture again to add a timeline. **Capture again** retains the prior reference until you accept a replacement. **Clear reference** unlocks the band layout. Tab moves focus, Enter or Space activates buttons, and Shift plus an arrow key makes larger stepper adjustments.

## Build on macOS

Set `VST3_SDK_DIR` to a complete Steinberg VST3 SDK checkout. Run local checks and package an optimized, ad-hoc signed development bundle with:

```sh
./scripts/ci.sh
./scripts/plugin-release dev --variant mxh
```

Copy the resulting `dist/mxh-<build>.vst3` bundle to `~/Library/Audio/Plug-Ins/VST3/MXH.vst3` and rescan VST3 plug-ins in the DAW. The development bundle is for local testing.
