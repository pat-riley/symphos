# Contributing to Symphos

Thanks for helping improve Symphos. The project is in an early proof-of-concept
stage, so small changes with clear behavior and measurements are the easiest to
review.

## Before making a change

- Read `docs/architecture.md` before changing the audio or analysis paths.
- Check `docs/roadmap.md` when proposing scene, mapping, or shader APIs.
- Keep the PipeWire process callback free of allocation, logging, locks, and
  analysis work.
- Discuss broad public APIs before building multiple implementations around
  them; scene-file compatibility will matter once users begin saving work.

## Local workflow

Create a branch from `main`, make a focused change, and run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

For audio or rendering changes, also test inside a normal graphical session and
include the source type, sample rate, display refresh rate, CPU architecture,
and GPU driver in the pull request.

## Pull requests

Explain the user-visible behavior, the reason for the change, and how you
verified it. Include before-and-after performance measurements when a change
touches a real-time or per-frame path. Keep formatting-only changes separate
from behavior changes where practical.
