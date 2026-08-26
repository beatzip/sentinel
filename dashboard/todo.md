# Project TODO

- [x] Add an isolated `approximate_spatial` functional consumer to the replay viewer.
- [x] Visualize generic fallback capsules, observed crouch state, and provenance without evidence or verdict controls.
- [x] Confirm the UI never reads approximate records into the exact evidence rail or LOS counters.
- [x] Add/extend Vitest coverage for the approximate spatial UI contract.
- [x] Connect demo 3 replay sidecar through a read-only dashboard data path.
- [x] Add a read-only player filter for real replay frames and approximate geometry.
- [x] Add a tick inspector that exposes frame, crouch and provenance without evidence actions.
- [x] Validate the real demo 3 replay in the viewer, then run Vitest, TypeScript and production build.
- [x] Add player-list search and quick selection without modifying replay data or evidence state.
- [x] Add an observed-crouch frame indicator and next/previous crouch navigation for the selected player.
- [x] Add read-only capsule hover details from `approximate_spatial` with provenance and geometry fields.
- [x] Validate the new functional-only controls on real demo 3, then run Vitest, TypeScript and production build.
- [x] Add the current-track definition-5037 downsampled sidecar through the existing functional replay consumer.
- [x] Validate nested non-evidentiary provenance before rendering the current-track generic capsule layer.
