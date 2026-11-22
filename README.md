# MSX

<a href='https://ko-fi.com/F7X2225YCF' target='_blank'><img height='36' style='border:0px;height:36px;' src='https://storage.ko-fi.com/cdn/kofi6.png?v=6' border='0' alt='Buy Me a Coffee at ko-fi.com' /></a>

MSX is a macOS VST3 reference meter built with Toybox. It measures band levels while passing mono or stereo audio through at unity with zero latency.

1. Insert MSX on a bus that can receive your reference track and your mix.
2. Set 2–8 bands and adjust their crossovers. Click the frequency steppers, drag a frequency vertically, or focus it and use the arrow keys.
3. Click **Capture reference**, play a representative section, and click **Use reference**. Capture at least one second of audible material.
4. Switch the bus to your mix. Each band shows the live level as a blue bar and the fixed trigger threshold as a teal line on the same dBFS scale. The threshold includes the captured peak, target offset and 1 dB trigger margin. Raise a mix element until orange appears above the line, then back it off until the orange fill clears. An orange marker and the **Peak over** reading hold the highest overage. Set **Peak hold** from 100 to 2,000 ms in 100 ms steps. The default is 500 ms and the setting saves with the project or preset.
5. Use **Target offset** to align the reference with your gain staging. Save the session or a host plug-in preset to reuse the reference.

The peak is the highest smoothed RMS level reached in each band during the capture. You can use the reference anywhere in the project or in another session. Existing references retain their peaks on load. **Capture again** retains the prior reference until you accept a replacement. **Clear reference** unlocks the band layout. Tab moves focus, Enter or Space activates buttons, and Shift plus an arrow key makes larger stepper adjustments.

MSX retains the VST3 class IDs and reads the settings and captured references from projects saved with MXH.

## Build on macOS

Set `VST3_SDK_DIR` to a complete Steinberg VST3 SDK checkout and run `./scripts/ci.sh` to check and build the optimized library.

Maintainers with access to the pinned Plugin Release toolkit can package an ad-hoc signed development bundle with:

```sh
./scripts/plugin-release dev --variant msx
```

Copy the resulting `dist/msx-<build>.vst3` bundle to `~/Library/Audio/Plug-Ins/VST3/MSX.vst3` and rescan VST3 plug-ins in the DAW. The development bundle is for local testing.
