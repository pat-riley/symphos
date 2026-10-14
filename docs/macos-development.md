# Mac setup and validation

Symphos now has a local Apple Silicon developer build that visualizes system
playback through Core Audio taps. It retains the shared egui/wgpu dashboard and
requires no virtual audio driver. Linux keeps the existing PipeWire backend.
This is the first desktop milestone; Ableton/VST3 work is a separate milestone.

## Build and launch

Use macOS 14.2 or newer, Xcode command line tools, and Rust 1.95 or newer. The
current lockfile and stable toolchain are used by the build. From the repo:

```sh
./scripts/build-macos.sh
open target/macos/Symphos.app
```

The script also finds Cargo in `~/.cargo/bin` if that directory is not on PATH.
It sets the deployment target, builds with the lockfile, copies the binary and
Info.plist into `target/macos/Symphos.app`, and verifies an ad-hoc signature.
Close the running app before rebuilding/relaunching; an existing process does
not update when the file changes. No `/Applications` installation is required.

This local signature is not Developer ID release signing or notarization. Do
not present this artifact as a ready-to-distribute public download. The native
app icon and installer/release automation remain follow-up packaging work.

## Audio behavior

The source picker offers **System audio** on Mac. It uses a nonmuting stereo
process mix and a private aggregate device; it does not replace the default
output, mute playback, or select a microphone. Analysis stays local. Rolling
waveform/frequency history lives in memory, with no audio recording file or
upload feature.

Allow Symphos when macOS requests system audio recording. The permission is
managed in **System Settings → Privacy & Security → Screen & System Audio
Recording**; exact wording varies by OS version. If needed, enable Symphos there
and choose **Settings → Retry audio capture** inside the app, or quit and reopen
it. Rebuilding an ad-hoc signed app may require granting access again. This occurred
during local validation: macOS logged a code-requirement mismatch, requested
audio permission again, and capture resumed after that permission flow completed.

Play ordinary audio through the computer to test it. Silence can be legitimate:
a green capture indicator means callbacks are arriving, not that the incoming
samples are nonzero. If a stream stops delivering buffers, the status changes
to waiting. Error details appear in **Settings → Issue log** and in the bounded
`~/Library/Logs/Symphos/issues.log` when the directory is writable.

Tap-format and default-output notifications trigger a fresh capture session.
The retry action also discards the old capture/history while retaining analysis
controls. Mac microphone/interface inputs, per-app filtering, and Windows
capture are not implemented yet. Protected or exclusive audio paths are not
promised by this developer preview.

## Validation on 14 October 2026

| Check | Result |
| --- | --- |
| macOS 26.6.1, arm64, Rust 1.99.0 | Local compile, Clippy, and release app build passed |
| Mac automated suite | 123 passed, 2 existing manual benchmarks ignored |
| Native buffer boundary | Tests cover interleaved/planar stereo, nonfinite samples, bounded overflow and sample counters |
| Reconnect analysis | Test verifies new sample rate and clean samples with shared analysis controls |
| Live app | Dashboard rendered with system source at 48 kHz |
| Real system playback | Quiet stereo tones at 440 Hz left / 880 Hz right appeared in waterfall, spectrum, waveform, spectrogram and meters |
| Manual retry | Reopened capture through Settings and repeated the stereo playback test successfully |
| Linux Ubuntu 24.04 arm64, PipeWire 1.0.5 | Clippy, release build, and 121 tests passed; 2 existing manual benchmarks ignored |
| CI configuration | Ubuntu 24.04 and macOS 14 jobs added; hosted workflow has not run from these local changes |

Before public release, complete fresh permission grant/deny/revoke tests,
Bluetooth/USB/default-device and sample-rate changes, sleep/wake, prolonged
playback, native Intel testing if offered, signed/notarized clean-machine
installation, and a real Omarchy graphical-session regression run. A Linux
container verifies compilation and tests, not desktop audio or the Asahi GPU.
The local playback test does not establish all those scenarios.
