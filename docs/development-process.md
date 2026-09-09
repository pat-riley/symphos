# Development, review, and release process

This document describes how Symphos can add review, testing, and compatibility
discipline without imposing mature-project ceremony during early exploration.
It is a direction for the project, not a set of required merge gates today.

Code review and automated tests solve different problems. Pull-request review
checks intent, architecture, usability, risk, and whether the evidence matches
the change. Tests repeatedly check behavior that can be specified in code. As
Symphos matures, important changes should normally receive both.

## Current phase: move quickly and leave evidence

Symphos is a proof of concept. The current process is intentionally advisory:

- Use a focused pull request for a meaningful feature, architectural change,
  or risky fix when practical. Small experimental work does not need approval
  ceremony merely to move forward.
- Describe the user-visible result, important tradeoffs, and verification in
  the pull request. Review the diff once from the perspective of a future
  maintainer before merging it.
- Let the existing CI workflow run formatting, Clippy, unit tests, and a release
  build. A failure should be understood before merge, but branch protection and
  required approvals are intentionally deferred.
- Add focused tests for stable logic, regressions, serialization, and subtle
  state transitions. Experiments may begin with manual evidence and acquire
  tests once their behavior settles.
- Record user-visible changes under `Unreleased` in `CHANGELOG.md`.

This phase should change when multiple contributors regularly overlap, releases
begin, or regressions become more expensive than the time saved by informal
merges.

## Review according to risk

Review depth should follow the consequence of a defect rather than the number
of changed lines.

### Low risk

Examples include documentation, copy, isolated styling, and internal cleanup
with unchanged behavior. A diff review and relevant automated checks are
normally sufficient.

### Normal risk

Examples include UI behavior, analyzer calculations, component settings, and
new visual modes. Review should cover the intended behavior, focused tests,
manual evidence where needed, error states, and interaction with saved state.

### High risk

Examples include the real-time audio callback, threading, unsafe code, render
hot paths, project-file schemas, migrations, shader contracts, platform
backends, packaging, and release automation. These changes should eventually
require another reviewer, targeted regression tests, and performance or
compatibility evidence appropriate to the risk.

For every nontrivial review, ask:

1. Is the behavior and non-goal clear?
2. Does the change preserve the runtime-lane and real-time safety contracts?
3. What fails when a device, file, shader, or GPU operation is unavailable?
4. Could it invalidate saved projects, presets, settings, or public contracts?
5. Is repeated work occurring in an audio callback or per-frame path?
6. Is the verification strong enough to catch this defect again?

## Test strategy

Tests should be added where they provide durable confidence, not to maximize a
coverage percentage.

- **Unit tests:** parsing, math, mappings, state transitions, migrations, and
  deterministic component behavior.
- **Integration tests:** audio-engine lifecycle boundaries, project/preset
  round trips, malformed input, device changes, and renderer-independent scene
  behavior.
- **Golden and compatibility fixtures:** representative files written by each
  supported project schema, including the oldest version still supported.
- **Performance tests:** capture latency, dropped samples, analysis throughput,
  frame time, allocation, and sustained memory behavior. Initially record
  measurements; enforce budgets only when the benchmark environment is stable.
- **Manual checks:** real audio devices, application/loopback routing, GPU and
  display behavior, accessibility, installers, and platform permissions.

A bug fix should include a regression test when the failure can be reproduced
reliably below the hardware or desktop-integration layer.

## Adoption stages

### Stage 0: advisory development

This is the current stage. Keep CI visible, use the pull-request template, add
high-value tests, and avoid required approvals or broad coverage targets.

### Stage 1: protect the main branch

Adopt this when contributors overlap or `main` is expected to stay releasable:

- Merge through pull requests.
- Require the existing CI check before merge.
- Require review for high-risk areas and changes from outside contributors.
- Add ownership guidance only where specialist review is genuinely needed.
- Prefer small, reversible pull requests and squash or otherwise keep history
  understandable.

### Stage 2: release candidates

Adopt this when downloads are published on a cadence:

- Define supported OS, architecture, GPU, audio-backend, and file-version
  combinations.
- Run a platform CI matrix and smoke-test packaged artifacts.
- Create a release candidate or short stabilization period for meaningful
  releases.
- Verify performance budgets, project migrations, clean installation, upgrade,
  and rollback or recovery behavior.
- Finalize the changelog, version, signed artifacts, and release notes from the
  same commit, then tag it immutably.

### Stage 3: stable compatibility guarantees

Adopt this before promising stable project files or a 1.0 public API:

- Version every persisted schema and maintain explicit migrations.
- Round-trip golden projects and presets from every supported version in CI.
- Treat shader inputs, plugin interfaces, CLI behavior, and documented project
  semantics as compatibility surfaces.
- Publish a deprecation window and migration path before removing a stable
  capability whenever feasible.
- Follow Semantic Versioning for public contracts and document exceptions.

## Breaking-change policy

Before 1.0, breaking changes are allowed while the design is being discovered,
but they must be intentional. A pull request that changes a project or preset
schema, shader contract, command-line interface, platform requirement, or other
documented behavior should:

- be labeled or called out as breaking;
- explain who or what is affected;
- include an automatic migration when practical;
- preserve a backup or fail without corrupting user data; and
- add a changelog entry and compatibility fixture.

Once stable releases begin, breaking public-contract changes should require a
major version unless compatibility can be retained through migration or a
deprecation period. Performance regressions that make a documented supported
configuration unusable should be treated with the same release-blocking care
as functional regressions.

## When to tighten the process

Review this document at each release milestone. Turn a recommendation into an
automated merge or release gate only when all three are true:

1. The protected behavior is important and sufficiently stable.
2. The check is reliable enough that false failures are uncommon.
3. The cost of a regression exceeds the ongoing cost of the gate.

This keeps process proportional: early work stays fast, while confidence grows
before users depend on saved projects, third-party integrations, and regular
releases.
