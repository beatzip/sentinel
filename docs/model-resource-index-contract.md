# Gate 1A.6 — Build-Scoped Model Resource Index Contract

## Purpose

This contract opens the identity boundary between observed demo telemetry and compiled Source 2 model bytes. It does **not** parse VMDL, skeletons, hitboxes, AG2, transforms, spatial crossings, or verdicts.

The Gate 1A.5 result for demo SHA-256 `e6fbdcd77267c6e38e9823fcaa4b37489e8e42b0894927aaf3e85c195ae363ae` is `handle_not_resolvable_from_demo`. The observed `m_hModel` is typed as `CStrongHandle<InfoForResourceTypeCModel>`, whose public schema representation is a pointer-like `m_pBinding`. Therefore Sentinel must not assume that a serialized 64-bit handle is a stable path hash, VPK key, CRC, filename-derived identifier, or offline-decodable resource ID.

> **Rule:** an offline installation inventory may verify a candidate resource's build provenance, canonical path, and SHA-256. It must never manufacture `CStrongHandle → path` rows without a separately auditable binding source.

## Evidence artifacts

The protocol deliberately separates a build-scoped resource inventory from a handle-binding capture. The two are joined only by exact canonical compiled path and SHA-256.

| Artifact | Authority | Permitted content | Not sufficient for |
|---|---|---|---|
| `model-resource-inventory.json` | Byte-verified CS2 installation / depot reconstruction | Build identity, canonical compiled paths, VPK entry provenance, resource SHA-256 | Resolving any handle by itself |
| `model-handle-binding.json` | Runtime ResourceSystem capture that exposes the serialized handle and resolved resource | Observed serialized handle, type, canonical path, resource SHA-256, capture provenance | Proving matching build bytes by itself |
| `verified-model-binding.json` | Validator output after both artifacts agree | Exact observed demo tuple plus the accepted binding | VMDL semantics or geometry |

An index producer must preserve raw capture data locally, record a redacted acquisition log, and publish only hashes, paths, build identifiers, tool version and verification results. Credentials, Steam request codes, process memory addresses and raw game payloads must not be published.

## Build-scoped resource inventory

`model-resource-inventory.json` is an immutable inventory for one explicitly named CS2 content build. `build_content_match` is `byte_verified` only when the inventory source bytes are proven against the declared historical manifest/reconstruction; a header number, patch label, size, CRC, filename or current-installation assumption is insufficient.

```json
{
  "schema_version": 1,
  "artifact_kind": "model_resource_inventory",
  "build": {
    "patch_version": "14175",
    "demo_build_num": "10847",
    "content_provenance": {
      "app_id": 730,
      "depot_id": "2347770",
      "manifest_id": "...",
      "build_content_match": "byte_verified",
      "redacted_log_sha256": "..."
    }
  },
  "resources": [
    {
      "resource_type": "CModel",
      "canonical_compiled_path": "safe/relative/path.vmdl_c",
      "resource_sha256": "64 lowercase hexadecimal characters",
      "vpk_entry": {
        "directory_index_sha256": "...",
        "archive_name": "pak01_NNN.vpk",
        "offset": 0,
        "length": 0
      }
    }
  ]
}
```

All paths must be safe relative `.vmdl_c` paths. Resource SHA-256 is computed over the extracted compiled-resource bytes. An inventory record may be `available` without being selected for any demo.

## Runtime handle-binding capture

`model-handle-binding.json` is the only permitted source of a `CStrongHandle → CModel` claim. It must show that the same serialization representation used by demo `m_hModel` was observed at the ResourceSystem boundary and resolved by that runtime to a canonical compiled resource. The capture format records handles as **decimal strings**: JavaScript numbers cannot exactly represent arbitrary 64-bit values.

```json
{
  "schema_version": 1,
  "artifact_kind": "model_handle_binding_capture",
  "capture": {
    "tool_version": "...",
    "capture_mode": "runtime_resource_system",
    "redacted_log_sha256": "...",
    "build_identity": {
      "patch_version": "14175",
      "demo_build_num": "10847",
      "content_provenance_sha256": "..."
    }
  },
  "bindings": [
    {
      "serialized_handle": "9371194797796759017",
      "resource_type": "CModel",
      "canonical_compiled_path": "safe/relative/path.vmdl_c",
      "resource_sha256": "64 lowercase hexadecimal characters",
      "binding_observation": "resource_system_resolve"
    }
  ]
}
```

A capture from an unknown content build, a capture that cannot establish serialized-token equivalence, a path without a resource hash, or a memory-only pointer dump is not an accepted binding.

## Session scope and handle stability

No public source establishes that `CStrongHandle<InfoForResourceTypeCModel>` values remain stable across process sessions, resource unload/reload cycles or runtime table reuse. Sentinel therefore treats the numeric handle as **session-scoped until proved otherwise**. Every binding capture must include a non-secret `capture_session_id`, capture start/end timestamps, a capture-artifact SHA-256 and the demo SHA-256 when a demo is recorded in that same capture session.

For an already-recorded demo, a later capture from the same nominal build is not a verified binding merely because its decimal handle happens to equal the demo value. It is `session_unlinked` and cannot enter VMDL or geometry work. A `verified` decision requires a same-session demo-to-capture link, or a separate reviewed stability study that proves the precise serialized-handle behavior across independent sessions and explicitly states the remaining limitations. Such a study is evidence about the studied build behavior; it does not retrospectively qualify an unrelated historic demo session by default.

The capture schema therefore extends as follows:

```json
{
  "capture": {
    "capture_session_id": "non-secret UUID",
    "demo_sha256": "required for same-session binding",
    "session_link": "same_session | session_unlinked",
    "handle_stability": "unproven | reviewed_cross_session"
  }
}
```

Only `same_session` plus all other verified-binding checks permits `verified`. `session_unlinked`, `unproven`, changed resource identity or observed slot reuse must return a non-evidence status and block the exact pipeline.

## Runtime acquisition safety boundary

Gate 1A.7 does not authorize creation of process hooks, DLL injection, process-memory reads, anti-cheat bypasses, or interaction with a VAC-protected CS2 process. Valve’s Trusted Mode documentation states that third-party files are blocked from interacting with CS2 by default and that targeted process tampering may be subject to VAC enforcement; it also states that there is no safe-injection whitelist. See https://help.steampowered.com/en/faqs/view/09A0-4879-4353-EF95 and https://help.steampowered.com/en/faqs/view/571A-97DA-70E9-FF74.

Accordingly, **no runtime capture mechanism is currently admitted**. Gate 1A.7 remains design-only until one of these conditions is independently established: an official Valve-supported export exposes the required binding, or Valve explicitly authorizes a non-VAC test environment and the capture method for this purpose. A user assertion that a context is isolated, offline or insecure is not by itself sufficient to activate tooling. Until then, the only safe acquisition path is the read-only build-matched resource inventory, which remains insufficient to resolve a handle by itself.

## Verified binding decision

The validator accepts a binding only when all of the following are true: the demo SHA-256 equals the observed-demo record; the tuple `(model_handle, hitbox_set, pose_recipe_version)` was actually observed; the binding capture has the same decimal serialized handle and resource type `CModel`; the inventory is `byte_verified` for the exact demo build/content boundary; canonical compiled path and SHA-256 agree between capture and inventory; the extracted `.vmdl_c` bytes hash to the accepted SHA-256; and all artifact paths are safe and non-duplicated.

```json
{
  "schema_version": 1,
  "artifact_kind": "verified_model_binding",
  "decision": "verified",
  "demo_sha256": "...",
  "observed_identity": {
    "model_handle": "9371194797796759017",
    "hitbox_set": 0,
    "pose_recipe_version": 2
  },
  "resource": {
    "resource_type": "CModel",
    "canonical_compiled_path": "safe/relative/path.vmdl_c",
    "resource_sha256": "..."
  },
  "inventory_sha256": "...",
  "binding_capture_sha256": "..."
}
```

The only other decisions are `unresolved`, when no qualifying binding exists; `conflict`, when two otherwise admissible sources disagree on path, type, hash or build identity; and `session_unlinked`, when the runtime binding is not demonstrably from the demo’s own capture session. All three decisions prohibit VMDL inspection, fixture qualification, skeleton/hitbox parsing, AG2 decoding and exact spatial evidence.

## Minimal acquisition sequence

First, reconstruct or otherwise obtain byte-verifiable content for the target build and produce the resource inventory. Second, obtain a ResourceSystem capture that proves how the runtime serialized token resolves to a `CModel` path and resource hash for the same build. Third, validate the capture against the inventory. The existing `VerifiedModelMappingManifest` intake in `sentinel-cli` already provides a useful downstream pattern for observed-tuple matching, safe `.vmdl_c` path validation and resource SHA-256 verification; Gate 1A.6 adds the missing independent provenance requirements before that downstream intake may be populated.

No model resources, fixture artifacts or synthetic index entries are created by this contract.
