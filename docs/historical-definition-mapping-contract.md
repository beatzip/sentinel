# Historical Definition-to-Model Mapping Contract

## Purpose

This contract defines the minimum evidence needed to promote an observed historical controller character definition into a build-qualified `definition_index → model_player` record. It does **not** resolve `m_hModel`, VMDL resources, bones, hitboxes, AG2, or geometry.

## Required artifact

```json
{
  "schema_version": 1,
  "demo_identity": {
    "sha256": "...",
    "patch_version": "...",
    "build_num": 10847
  },
  "build_evidence": {
    "artifact_kind": "manifest_reconstructed_items_game | versioned_official_export | reviewed_build_archive",
    "build_binding": "verbatim source record tying artifact to demo build",
    "source_path": "scripts/items/items_game.txt",
    "source_sha256": "...",
    "provenance_sha256": "..."
  },
  "definition_mapping": {
    "definition_index": 5308,
    "prefab": "...",
    "model_player": "..."
  },
  "review": {
    "approved": false,
    "evidence_references": ["..."]
  }
}
```

The artifact must provide an auditable build binding and byte hash for the historical `items_game.txt`, then parse the literal definition record from that same byte sequence. A public tracker commit, a current install, an unverified VPK, asset namespace similarity, a filename, CRC, definition-index stability, or an `m_hModel` value alone is insufficient.

## Outcomes

| Outcome | Meaning |
|---|---|
| `verified_definition_to_model_mapping` | Every required field is present, byte-qualified, build-bound, and reviewer-approved. This remains distinct from `m_hModel → path`. |
| `unresolved_no_build_qualified_items_game` | No historical `items_game.txt` with a verified build binding is available. |
| `conflict` | Candidate artifacts disagree on any build, hash, definition, prefab, or path field. |

Even `verified_definition_to_model_mapping` cannot admit VMDL parsing, fixture qualification, AG2, hitbox processing, or exact geometry without their separately required evidence gates.
