# AG2 Pose и Source2 Hitbox Resources — исследовательские ориентиры

Дата: 2026-08-19.

## Подтверждённое внешними источниками

- Valve Developer Community описывает hitbox sets как данные Source 2 VMDL, но сайт защищён Anubis и не дал прочитать подробную документацию из sandbox: https://developer.valvesoftware.com/wiki/VMDL/Hitboxes
- ValveResourceFormat — открытая библиотека для разбора Source 2 resource files; её список поддерживаемых типов включает `vmdl`, `vanim`, `vagrp`, `vphys` и `vnmskel`: https://github.com/ValveResourceFormat/ValveResourceFormat

## Ограничение Sentinel

Наблюдаемые props demo дают `model_handle`, `hitbox_set`, `anim_graph_id` и AG2 pose recipe version, но не дают resource path, decoded hitbox geometry или world-space bone transforms. До сопоставления `model_handle -> compiled model resource` и независимой проверки pose decode Sentinel обязан выдавать `hitboxes` как unavailable/unsupported.

## Проверяемый минимальный следующий шаг

Не писать общий Source2 asset engine. Сначала добыть одну локальную, воспроизводимую пару: snapshot из demo с pose bytes и соответствующий compiled player model asset из той же версии игры. Использовать её как golden fixture для reverse engineering и unit tests.

## Минимальная geometry, которую должен отдавать loader

Открытая реализация ValveResourceFormat читает hitbox sets в mesh resource из `m_hitboxsets`; внутри встречаются оба варианта ключа: `m_HitBoxes` и `m_hitboxes`. Каждый `CHitBox` содержит `m_sBoneName`, `m_vMinBounds`, `m_vMaxBounds`, `m_flShapeRadius`, `m_nShapeType`, `m_nGroupId`, `m_bTranslationOnly` и surface property. Model resource может получить hitbox sets из подключённого external mesh resource.

Следовательно, первый контракт Sentinel должен хранить immutable local-space hitbox geometry плюс `bone_name`, но не вычислять world-space shape, пока не получена проверенная матрица именно этой кости на точно совпадающем tick.

Источник кода: https://github.com/ValveResourceFormat/ValveResourceFormat/blob/master/ValveResourceFormat/Resource/ResourceTypes/Mesh.cs и https://github.com/ValveResourceFormat/ValveResourceFormat/blob/master/ValveResourceFormat/Resource/ResourceTypes/ModelData/Hitbox.cs

## Текущее parser ограничение

`source2-demo` 0.5.8 в `FieldValue` поддерживает только scalar integer/float, boolean, string и Vector2D/3D/4D; bytes или variable arrays отсутствуют. Поэтому текущий `Entity::get_property()` не позволяет извлечь `m_SerializePoseRecipeAG2Dynamic` как raw byte payload. Первый engineering gate — доказать, что нужное поле существует в raw PacketEntities и расширить или заменить слой decoding entity fields, а не писать AG2 decoder поверх несуществующих байтов.

Источник: https://docs.rs/source2-demo/0.5.8/source2_demo/enum.FieldValue.html

## Предлагаемый минимальный data contract

1. `ObservedPoseRecipe`: `demo_hash`, `tick`, `pawn_entity_index`, `model_handle`, `hitbox_set`, `anim_graph_id`, `pose_recipe_version`, `raw_bytes_sha256`, `raw_bytes` (только в локальном artifact; не сериализовать в публичный replay по умолчанию).
2. `ModelHitboxSet`: `asset_hash`, `asset_path`, `set_index_or_name`, `skeleton_hash`, `hitboxes[]`. Каждый hitbox — локальная geometry из ресурса с `bone_name`, shape type, bounds/radius, group и surface property.
3. `DecodedPose`: `decoder_version`, ссылки на `ObservedPoseRecipe` и `ModelHitboxSet`, `bone_local_transforms[]`, `validation_status`. До успешной validation world transforms отсутствуют.
4. `WorldHitboxSnapshot`: `tick`, `player_id`, `origin`, `orientation`, `bone_world_transforms[]`, `hitboxes[]`, `availability`. Создаётся только после exact version/identity match всех предыдущих слоёв.

## Обязательные quality gates

- Отклонять запись, если отсутствует raw AG2 payload, asset path/hash, выбранный hitbox set, bone из skeleton, одинаковый tick или подтверждённый world transform.
- Сверять decoded bone count/names с skeleton model resource и затем сверять минимум одну воспроизводимую позу с независимым визуальным Source2 reference.
- Помечать любой mismatch как `unavailable`; никогда не заменять его standing/ducking capsule или inference из eye offset.

## Порядок реализации

### Gate 0 — сделать pose bytes доступными

Сначала воспроизводимо вывести property type, размер и SHA-256 `m_SerializePoseRecipeAG2Dynamic` на одном pawn/tick в локальный ignored trace. Если `source2-demo` не предоставляет variable array, минимальное изменение — fork/patch только его entity-field decoding, чтобы сохранить `Vec<u8>` для этого поля. Не менять replay schema и не писать AG2 math до этого gate.

### Gate 1 — получить model resource fixture

Сопоставить зафиксированный `model_handle` с конкретным `.vmdl_c`/external `.vmesh_c` asset из локальной CS2 install той же версии. Добавить local-only fixture manifest с SHA-256 demo, asset и pose bytes; не коммитить game assets. Asset loader должен вернуть только local-space hitbox sets и skeleton names.

### Gate 2 — offline decode и validation

Создать pure decoder `bytes + model skeleton -> bone-local transforms` без доступа к demo/replay. Golden test должен проверять byte hash, decoder version, bone count, bone-name mapping и хотя бы одну independently inspected pose. Невалидные и незнакомые версии должны возвращать structured unavailable result.

### Gate 3 — world transforms и hit test

Сочетать local bone transform с игроковым origin/orientation только на matching tick. После matrix composition преобразовать geometry из `ModelHitboxSet` и применить segment-vs-box/sphere/capsule test. Сохранять hitbox ID, bone name, tick, asset hash, pose hash и exact intersection parameter как evidence.

### Gate 4 — включить evidence, не verdict

Только после regression fixtures из Gate 2–3 разрешить `hitbox_intersection` как отдельный available spatial capability. Сначала его использовать для объяснения linkage/TTD; не использовать самостоятельно для triggerbot/aimbot/cheat verdict.

## Gate 1 asset audit — 2026-08-20

Проверка предоставленных assets дала следующие факты:

- `pak01_dir.vpk` и `ctm_diver_varianta.vmdl_c` согласованы по CRC-32 `086fed19`; это подтверждает происхождение descriptor из данного VPK index.
- Index показывает 83 player descriptor в `pak01_262.vpk` и 2 в `pak01_407.vpk`; оба segment files предоставлены и read-only extraction подтверждён.
- Все извлечённые `characters/models/ctm_*` и `tm_*` player descriptor имеют размер примерно 4.8 KiB. Проверенный `ctm_diver_varianta.vmdl_c` содержит один `dummy` bone и пустой `m_hitboxsets`.
- Archive содержит другие VMDL с non-empty hitbox keys, но без independently proven `m_hModel -> asset path` они не являются допустимым geometry source для player snapshots.
- Три наблюдаемых `m_hModel` values не совпадают с низшими 32 битами VPK CRC; такая эвристика не используется как mapping.

Следующий точный blocker: найти demo- или game-side model precache/resource manifest, который связывает runtime `CStrongHandle` с конкретным compiled model resource. До этого loader обязан возвращать `unavailable` для hitbox geometry, даже при наличии несвязанных VMDL geometry в archive.

### Gate 1 verified-manifest metadata gate — implemented

`sentinel replay` now accepts an explicit local-only option:

```text
sentinel replay <match.dem> [output.replay.json] --verified-model-manifest mapping.json
```

The manifest is accepted only when it uses `schema_version: 1`, contains the SHA-256 of that exact demo, gives a non-empty game-build declaration, and provides at least one unique observed non-zero-player `(model_handle, hitbox_set, pose_recipe_version)` triple. Each `asset_path` must be a safe relative `.vmdl` path and each `resource_file` must equal its compiled `.vmdl_c` path. Sentinel canonicalizes the resource file inside the manifest directory, rejects path traversal/symlink escape, and verifies the declared SHA-256. A mismatch, unobserved handle, duplicate identity, missing resource, unsafe path, unknown JSON field, or path/extension mismatch rejects the export rather than emitting verified identity metadata.

Accepted entries are emitted only as `verified_model_mappings` replay metadata with `mapping_source=verified_manifest` and `build_verification=external_manifest_declaration`. The adapter does not currently expose a verified CS2 build identity, so a manifest game-build value is not proof that the demo and resource build match. `observed_model_identity_count` and `model_mapping_coverage` explicitly report `unavailable`, `partial`, or `complete` identity coverage. `complete` means only that every currently observed player identity has a verified manifest record; it is **not** model geometry, a decoded pose, hitbox intersection, LOS, penetration, or a verdict. Without an explicit valid manifest, `verified_model_mappings` is empty; generic fallback and `approximate_spatial` are unchanged.

The current uploaded VPK index, segments, and extracted descriptors still do not provide that manifest, so no real demo mapping has been promoted. Exact geometry remains unavailable even after `complete` metadata coverage: Gate 1 geometry needs VMDL/mesh/skeleton parsing and verified hitbox sets; Gate 2 needs a byte-level AG2 fixture and decoder; Gate 3 needs golden expected bone transforms. Neither decoder, world hitbox geometry, intersection evidence, nor approximate LOS is enabled by this gate.

### Gate 1 string-table audit

В реальной demo `source2-demo` string-table callbacks подтвердили только `genericprecache`, `AnimAssetData`, `instancebaseline`, `userinfo` и служебные tables. `genericprecache` содержит пустой row; `AnimAssetData` содержит chicken/world/weapon animation graph и skeleton paths, но не player model path. `modelprecache` callback не получил entries. Следовательно, текущая demo не предоставляет доказуемый runtime `m_hModel -> player VMDL path` mapping через string tables.

Это отрицательное наблюдение важно: index строки, VPK CRC и похожие geometry не должны использоваться как replacement mapping. Для продолжения Gate 1 нужен отдельно сохранённый resource-handle manifest от той же CS2 game build либо независимый reference, который связывает каждый observed 64-bit handle с конкретным compiled model resource.

### Gate 1B resource dependency discovery — implemented

`sentinel-model` now parses the documented Source 2 compiled-resource header and block directory for a local `.vmdl_c`. It deterministically records the resource SHA-256, `header`, and for every block its tag, offset, raw stored size, raw SHA-256, bounded structural signatures, and bounded raw resource-path-like strings. The read-only command is:

```text
sentinel model-describe player.vmdl_c [output.resource.json] [--asset-root directory]
```

`RERL` is parsed structurally as the authoritative external-reference list and each listed resource receives a dependency type/path plus explicit `resolved`, `missing`, or `unsafe_path` status below `--asset-root`. The container descriptor still treats `REDI`/`RED2` and `DATA` as non-semantic: it recognizes a bounded Binary KV3 header signature and, if a complete header is present, its declared compression mode and declared compressed/uncompressed byte counts. It scans at most 8 KiB and returns at most 32 printable path-like byte strings; those strings are diagnostic hints, not dependencies or proof of embedded geometry.

No generic skeleton, generic hitboxes, inferred model dependency, AG2 pose, world transform, or geometry claim is substituted.

Validation on the supplied `ctm_diver_varianta.vmdl_c` discovered `MVTX`, `MIDX`, `MDAT`, `CTRL`, `RERL`, `RED2`, and `DATA` blocks and one missing material reference. It did not discover a mesh or skeleton dependency, so the artifact truthfully remains geometry-unavailable.

The capabilities are intentionally separate:

```text
Gate 1A: model identity resolution (handle -> resource identity)      metadata-only
Gate 1B: VMDL container/dependency discovery                          implemented
Gate 1C.1: bounded Binary KV3 v5 generic tree decode                   implemented
Gate 1C.2: read-only VMDL-shaped semantic-key inspection               implemented
Gate 1D: deterministic skeleton/hitbox-set semantic parser             requires verified fixture bundle
Gate 1E: exact local-space model geometry snapshot                     requires Gate 1D
Gate 2:  AG2 pose decoding and bone-local transforms                   unavailable
Gate 3:  exact world-space hitboxes                                    unavailable
Gate 4:  exact hitbox spatial evidence                                 unavailable
```

Model identity resolution is not exact geometry, and exact geometry is not exact spatial evidence. `sentinel-model` is isolated from `generic_fallback`, `approximate_spatial`, `SpatialShotEvidence`, and verdict code until later gates have verified fixtures.

### Gate 1C.1 / 1C.2 Binary KV3 decode and semantic inspection — implemented

`sentinel-model` now decodes a bounded generic Binary KV3 v5 tree for uncompressed and standard 16 KiB LZ4 buffers. Zstd payloads and Binary KV3 blob streams return explicit unsupported results in this initial decoder; neither condition is silently approximated. `sentinel model-kv3-inspect player.vmdl_c <MDAT|CTRL|RED2|DATA> [output.json]` writes a local inspection artifact with the raw block descriptor, generic tree, and a bounded report of only these names when present: `m_skeleton`, `m_modelSkeleton`, `m_hitboxsets`, `m_bones`, `m_boneName`, and `m_nParent`.

The decoder was independently compared with the ValveResourceFormat oracle on the supplied ignored `ctm_diver_varianta.vmdl_c`: all four v5 blocks (`MDAT`, `CTRL`, `RED2`, `DATA`) decode. The local golden regression confirms the oracle facts that `MDAT._class` is `CRenderMesh`, `MDAT.m_hitboxsets` is empty, and `DATA.m_modelSkeleton` carries only the `dummy` name and parent `-1`. The inspection still writes `exact_geometry_available=false`; it does not construct a parsed skeleton artifact, interpret a hitbox shape, resolve model identity, decode AG2, or produce spatial evidence.

### Gate 1D.0 fixture qualification — `not_geometry_fixture`

The full decoded local JSON for `MDAT`, `CTRL`, `RED2`, and `DATA` was reviewed, rather than relying on the bounded key summary. The concrete paths are `MDAT.root._class = CRenderMesh`, `MDAT.root.m_skeleton.m_bones[0]` with one `dummy` bone, `MDAT.root.m_hitboxsets = []`, and `DATA.root.m_modelSkeleton` with one `dummy` name, parent `-1`, zero translation, identity rotation, and unit scale. `DATA.root.m_refMeshes`, `m_refPhysicsHitboxData`, `m_vecNmSkeletonRefs`, and `m_refAnimGroups` are all empty. `CTRL` declares an embedded mesh table only. `RED2` identifies the VMDL compiler input and reports one bone with zero NM-skeleton and AG2 references.

The `RERL` list contains only the missing material `materials/pbr_defaults/default_orange001.vmat`; it declares no mesh, skeleton, or hitbox dependency. Independently, no local verified mapping artifact binds an observed demo `(model_handle, hitbox_set, pose_recipe_version)` tuple to this resource. Consequently this artifact is classified as `not_geometry_fixture`, not as a qualified skeleton or hitbox parser fixture. Gate 1D.1 must not treat a global key match as a parser schema, and is blocked for this resource. The ignored `fixture-qualification.json` records the exact reviewed paths and raw block hashes.

### Gate 1D.1 intake — qualified verified player-model fixture bundle

Gate 1D.1 does not accept a VMDL chosen by filename, a similar VPK asset, a CRC match, or a resource discovered by an asset-name search. Its only permitted intake is a local ignored bundle derived in this order: observed Sentinel player telemetry, the observed `(model_handle, hitbox_set, pose_recipe_version)` tuple, an existing verified-manifest acceptance for that exact demo, the mapped exact VMDL path, deterministic extraction, `model-describe`, `model-kv3-inspect`, and qualification.

```text
fixture/
├── manifest.json
├── model.vmdl_c
├── dependencies/
│   └── every actually declared resource
├── sha256.json
└── expected/
    └── qualification.json
```

`manifest.json` must preserve the accepted manifest’s demo SHA-256, declared game build, the one observed identity tuple, mapped asset path, asset SHA-256, and the fact that its mapping source is `verified_manifest`. It is not enough to restate those values: the source manifest must already have passed the existing `sentinel replay --verified-model-manifest` acceptance checks for the exact demo, which include the observed identity and compiled-resource hash checks. `model.vmdl_c` must hash to the mapped asset SHA-256. `sha256.json` lists the SHA-256 for `model.vmdl_c` and every extracted declared dependency. A missing declared dependency produces `requires_dependency`; a raw string hint never creates a dependency.

The required `expected/qualification.json` records the concrete typed context rather than a global key search:

```json
{
  "observed_identity": {
    "model_handle": "...",
    "hitbox_set": "...",
    "pose_recipe_version": "..."
  },
  "verified_asset": {
    "path": "...",
    "sha256": "..."
  },
  "qualification": {
    "resource_class": "...",
    "skeleton_schema_path": "MDAT.root.<typed object>.<field>",
    "hitbox_sets_schema_path": "<block>.root.<typed object>.<field>"
  }
}
```

Each non-null path is literal and block-qualified. It must resolve from the named decoded block’s root through the declared object fields, and not by recursively finding a matching key anywhere in the generic KV3 tree. The qualified skeleton path must lead to a real bone collection with its parent graph and bind/local transforms; the qualified hitbox-set path must lead to real hitbox definitions with a bone reference and bounds or capsule radius. Qualification may yield only `qualified_for_skeleton_parser`, `qualified_for_hitbox_parser`, `requires_dependency`, or `not_geometry_fixture`. The first two statuses permit a future parser scoped to those exact paths; the latter two do not.

### Gate 1D.2 — Model Schema Registry (post-fixture)

After — and only after — the first telemetry-derived fixture qualifies, Sentinel may record a deterministic schema fingerprint. A registry entry binds that fingerprint to the exact resource class, literal block-qualified skeleton and hitbox-set schema paths, parser version, fixture hash, resource hash and dependency-hash set used to establish it. The fingerprint is a parser-routing key, not a claim that all future resources are equivalent. Registry writes require explicit human review of the initial qualified fixture; no inferred or filename-selected asset may create an entry.

### Gate 1D.3 — Automatic model resolution (post-registry)

The production routing chain is strictly `observed m_hModel → accepted verified asset mapping → exact resource bytes → schema fingerprint → registered exact parser`. A known fingerprint selects only its registered parser and schema paths. An unknown, malformed, hash-mismatched or dependency-incomplete fingerprint produces structured `unsupported`; it must not fall back to a global KV3 key search, generic capsules, approximate geometry or exact spatial evidence. Successful parsing yields only deterministic local skeleton/hitbox artifacts; AG2 decoding and world-space evidence remain independently gated.

### Gate 0 capture status — passed locally; Gate 1 remains blocked

The local opt-in Source 2 trace now emits one real `CCSPlayerPawn` pose record with its tick, pawn entity index, controller slot, SteamID, observed `m_hModel`, `m_nHitboxSet`, AG2 active-slot value when exposed, pose-recipe version, byte length, SHA-256 and raw bytes. The raw payload is retained only in ignored local audit output and remains outside replay JSON, model mapping, geometry, spatial evidence and verdict flows.

Two independently supplied demos showed that `m_hModel`, `m_SerializePoseRecipeAG2Dynamic`, and `m_nSerializePoseRecipeAG2ActiveSlot` are present in SendTables. Each produced a non-empty real-pawn AG2 dynamic byte array, so the byte-capture prerequisite is passed. Their diagnostics did not expose a modelprecache-style table, player VMDL path, or a build-matched item/agent/econ schema source. That negative result is not converted into a mapping: Gate 1 remains blocked until the documented non-handle chain from observed agent/econ/loadout identity to a build-matched schema/VPK source and exact model resource is supplied.

The local trace records both raw `m_iItemDefinitionIndex` from the pawn and raw `m_nPawnCharacterDefIndex` from its resolved controller. They are deliberately not interchangeable. In the extracted build-specific `items_game.txt`, observed pawn item definition `5034` is `specialist_gloves` with prefab `hands_paintable`, proving that it is equipment telemetry rather than an agent identity. The controller definition is the permitted agent schema key: a same-tick observed controller value `5308` resolves to `customplayer_ctm_fbi_variantb`, prefab `customplayertradable`, and `agents/models/ctm_fbi/ctm_fbi_variantb.vmdl` in that exact schema. This is a schema-stage linkage only; it is not yet verified asset identity.

The supplied `build_agent_model_manifest.py` is appropriate for a narrow schema stage after its path filter is extended to include the observed `agents/models/` namespace as well as `characters/models/`. It parses a supplied `items_game.txt` and writes the source SHA-256 with `def_index → model_player` entries. It neither validates demo build matching nor reads VPK resources, resolves dependencies, verifies a VMDL hash, or cross-links a pawn/tick; it must therefore not itself be treated as a verified-model manifest. The actual VMDL entry is indexed in `pak01_003.vpk`, which was not supplied, so its bytes, SHA-256, container dependencies, and build provenance remain unavailable.

### Observed custom-agent VMDL candidate — extracted, not qualified

The required VPK chunk was later supplied. The observed same-tick controller value `m_nPawnCharacterDefIndex = 5308` leads through the extracted `items_game.txt` to `agents/models/ctm_fbi/ctm_fbi_variantb.vmdl`; its compiled resource was deterministically selected through `pak01_dir.vpk` at `pak01_003.vpk:88144544+939909` and extracted as `agents/models/ctm_fbi/ctm_fbi_variantb.vmdl_c`. Its resource SHA-256 is `37d673ef1cf8e1575b0a6e32f47dcdf14e42599ca22555c6db8ba9ac6acc4abf`.

The container has 40 blocks, including embedded `MVTX`/`MIDX` geometry buffers, seven Binary KV3 v5 blocks, and 14 declared RERL references. Its generic decoded `DATA.root.m_modelSkeleton` is a concrete schema candidate with equally sized `m_boneName`, `m_nParent`, `m_bonePosParent`, `m_boneRotParent`, and `m_boneScaleParent` collections of 94 entries. This records a possible future `DATA.root.m_modelSkeleton` skeleton-parser path; no ParsedModel is constructed now. `DATA.root.m_refPhysicsHitboxData` is an empty array, while `m_vecNmSkeletonRefs` explicitly names `animation/skeletons/characters/worldmodel.vnmskel` and `animation/skeletons/characters/viewmodel.vnmskel`. The generic `PHYS` root contains physics parts and bind-pose data, but it has not been interpreted as hitbox semantics.

This candidate remains `requires_dependency` and `qualified=false`: VPK co-presence is not evidence that its content build matches the demo header, and the RERL/NM skeleton resources have not been independently extracted, hashed, or qualified. Consequently there is no qualified hitbox-set path, no hitbox parser, no AG2 decoding, and no exact spatial evidence. The ignored local descriptor and generic inspection JSON preserve block hashes and the audit trail.

### Variant B historical manifest attempt — anonymous access blocked

For the user-specified app `730`, Windows depot `2347771`, and manifest `358563799254497787`, the official DepotDownloader 3.4.0 was first run in `-manifest-only` mode with an exact three-file list (`pak01_dir.vpk`, `pak01_003.vpk`, `pak01_491.vpk`) and no credentials. Steam returned no anonymous manifest request code and the Steam content CDN returned HTTP `401`; no historical VPK bytes were downloaded. This is an authorization boundary, not negative evidence about the candidate build.

The ignored `reconstruction-attempt/` bundle preserves the redacted anonymous log and explicit empty historical `sha256sums.txt`/index records. A user-side authenticated reconstruction may supply only its redacted log, manifest metadata, historical VPK/resource SHA-256 sums, and VPK index records. The historical bytes must then be compared against the local candidate; a mismatch keeps local `build_match=unproven`, while a match becomes one required component of fixture qualification.

### Variant B 19-Aug content check — directory index mismatch

The public 19-Aug Windows depot `2347771` manifest does not list any requested `game/csgo/pak01*` path, so it cannot compare the supplied VPKs. The corresponding public 64-bit depot `2347770` manifest `4814468113142569832` does list `game/csgo/pak01_dir.vpk` and was anonymously reconstructed. Its directory index is 7,586,006 bytes with SHA-256 `a5bf0956b4b13a217bf44400a9897e712a4402f2a791b9252f85d91d2d65d2e2`; the supplied local index is 7,586,034 bytes with SHA-256 `6af720c25e090b8b29f6a3e9d041bbcd25a861e4f9864b9e7f41ce1f85280819`.

The exact Windows command `-app 730 -depot 2347771 -manifest 4846265837652631529` was also run in anonymous `-manifest-only` mode. It successfully returned the 19-Aug-2026 23:15:54 manifest, but its 184-entry file list contains zero `pak01`-like entries and zero matches for `game/csgo/pak01_dir.vpk`, `pak01_003.vpk`, or `pak01_491.vpk`. This directly establishes that the manifest is not a VPK comparison source; it is neither the 12-Aug baseline nor a conflicting result for the 64-bit depot diagnostic.

The two directory-index files are not byte-identical. This proves that the supplied local index is not the exact 19-Aug `2347770` manifest index. No `pak01_003.vpk`, `pak01_491.vpk`, or other chunk was listed or downloaded in this comparison, so no claim about individual chunk equality is made. The local fixture remains unqualified for the target demo; the ignored `directory-index-comparison.json` keeps the precise scope.

This 19-Aug check is a separate local-client content candidate diagnostic; it does not replace the original 12-Aug baseline manifest `358563799254497787` in depot `2347771`. The manifest IDs are distinct and their public timestamps are distinct. The successful 19-Aug retrieval log records anonymous Steam3 login, depot key success, manifest request-code success, CDN HTTP 200, pre-allocation of the exact `game/csgo/pak01_dir.vpk` output path, eight successful chunk downloads, and `3,309,328` downloaded bytes / `7,586,006` uncompressed bytes. Its delivery copy redacts only the temporary request code; this code is not needed to verify the request, manifest, path, download count, or resulting hash.

### Variant B 12-Aug 64-bit manifest candidate — anonymous access blocked

The public metadata candidate for build `24701871` is app `730`, 64-bit depot `2347770`, manifest `7645176062026597595`, dated 12-Aug-2026 22:18:51 UTC. The official DepotDownloader 3.4.0 was run in anonymous `-manifest-only` mode for that exact app/depot/manifest. Steam3 anonymous login and the depot-key request succeeded, but Steam returned no anonymous manifest request code and the content CDN returned HTTP `401`; no file list, directory index, VPK archive bytes, or resource bytes were obtained.

Therefore the claimed association is recorded only as a historical candidate, not as verified VPK source content. In particular, Sentinel has not established whether this manifest lists `game/csgo/pak01_dir.vpk`, has not downloaded or hashed that directory index, has not derived any historical archive indices, and has not compared it to local VPK bytes. The 12-Aug build/content boundary remains `unproven` and is now **abandoned** for this task: no more retrieval, reconstruction, neighboring-manifest inference, parser work, or fixture qualification may claim or target `12aug_exact`.

### Variant B 12-Aug authenticated three-file attempt — failed before manifest access

One authenticated DepotDownloader attempt was made for app `730`, depot `2347771`, manifest `358563799254497787`, and the exact file list `game/csgo/pak01_dir.vpk`, `game/csgo/pak01_003.vpk`, and `game/csgo/pak01_491.vpk`. Steam3 connection and password submission completed, but the connection was lost during Steam Guard handling and DepotDownloader ended with `InitializeSteam failed`. The user directed that no new Steam Guard code, retry, SteamCMD fallback, or other retrieval method be attempted in this task.

No manifest request code, manifest date, CDN status, file listing, chunk GID, VPK byte, reconstructed hash, comparison result or VPK index record was produced. The ignored local `authentication-attempt.json` contains only the redacted scope and failure state `authentication_attempt_failed_no_bytes`; it contains no username, password, Steam Guard code, Steam request code or raw client log. This is an authentication failure record, not a historical content reconstruction. The 12-Aug baseline remains `unproven` and comparison input remains absent.

The user later explicitly authorized a renewed attempt and one final corrected-password attempt under the same scope. The renewed attempt failed with `InvalidPassword` before Steam Guard. In the final attempt, the redacted client console emitted successful password/Steam Guard markers and then `Got 2 licenses for account!`; it later emitted `Failed to poll status with result FileNotFound` and disconnected before any depot-key, manifest, file-list or download stage. This is an observed console-marker order only, not an inferred SteamKit2 internal authentication-causality claim. The verified output directory contains zero files. The ignored `final-authentication-attempt.json` is redacted and records `authenticated_client_failed_no_bytes`; it contains no account identifier, credential, Steam Guard value, request code or raw client log. No further authenticated retries or alternate retrieval paths are authorized in this task.

### Current content baseline — verified 19-Aug alternate directory index

The received `pak01_dir.vpk` is user-confirmed as originating from a fully updated local Steam CS2 installation. Independent local hashing measured `7,586,006` bytes and SHA-256 `a5bf0956b4b13a217bf44400a9897e712a4402f2a791b9252f85d91d2d65d2e2`. It is byte-identical to the previously reconstructed 19-Aug-2026 64-bit depot `2347770` manifest `4814468113142569832` directory index. A bounded VPK v2 check observed magic `0x55aa1234`, version `2`, declared tree size `6,185,016`, and parsed the `scripts/items/items_game.txt` record at archive `0x01eb`, offset `83,968,384`, length `8,382,779`.

This establishes the index as `verified_19aug_alternate_directory_index` with user-confirmed current-installation provenance. It is permitted only for `current` / `alternate_manifest` directory-tree inspection, archive-location derivation, extraction and content-pipeline validation. It never establishes `12aug_exact`, demo build matching, the historical observed `m_hModel` to resource path mapping, model fixture qualification, AG2 decoding or exact geometry.

The first current-scope chunk coherence check used the indexed `scripts/items/items_game.txt` entry. The index locates it in `pak01_491.vpk` archive index `0x01eb` at offset `83,968,384`, length `8,382,779`, with expected CRC-32 `0xda1564c5`. The available chunk is `100,733,991` bytes with SHA-256 `2a563a862b669948dc25204475f2e4c0827a2da567ec89c3b064dceb77cbd18f`; deterministic extraction produced exactly `8,382,779` bytes, SHA-256 `3005506d74a7e8d0dfad104f732c56db0e2555a0a340da6f0636efbb6d281a8d`, and matching CRC-32 `0xda1564c5`. This proves only directory-index/chunk coherence for that current-scope record. The ignored `items-game-coherence.json` preserves the bounded validation values and explicit exclusions.

The second current-scope coherence check used `agents/models/ctm_fbi/ctm_fbi_variantb.vmdl_c`. The index selects `pak01_003.vpk` archive `0x0003` at offset `88,144,544`, length `939,909`, with expected CRC-32 `0x15cb93de`. The available chunk is `118,745,676` bytes with SHA-256 `8f3b934e89f79c1d7d1680d485a60b050f6687336179778c435bf8884209d86e`; extraction produced exactly `939,909` bytes, SHA-256 `37d673ef1cf8e1575b0a6e32f47dcdf14e42599ca22555c6db8ba9ac6acc4abf`, and matching CRC-32 `0x15cb93de`. This proves only current directory-index/`pak01_003` coherence for this compiled model record. It does not bind the historical demo’s `m_hModel` to the path, qualify the VMDL as fixture, or permit semantic parsing/AG2/exact geometry. The ignored `current-player-model-coherence.json` preserves the bounded result.

### Current/19-Aug structural inspection — read-only

The validated current `items_game.txt` KeyValues document has 33 top-level sections, 2,054 item records (2,053 numeric keys), and 255 records with `model_player`. The model paths partition into 151 `agents/models`, 76 `models/props`, 26 `weapons/models`, and 2 `models/weapons` prefixes. The selected schema records preserve the known distinction: `5034` is `hands_paintable` with the specialist-glove VMDL, whereas `5308` is `customplayertradable` and names `agents/models/ctm_fbi/ctm_fbi_variantb.vmdl`. This is current schema structure only, not a mapping from any demo handle.

The corresponding extracted VMDL is a version-1/header-version-12 `vmdl` resource with 40 blocks: 17 `MVTX`, 9 `MIDX`, 5 `MDAT`, and one each `MRPH`, `ANIM`, `ASEQ`, `AGRP`, `PHYS`, `CTRL`, `RERL`, `RED2`, and `DATA`; 13 blocks carry bounded Binary KV3 v5 signatures. `RERL` lists 14 external resources, including two NM skeleton paths, but they are unresolved in the supplied current asset root. The decoded `DATA` block has a model name, hull bounds, empty `m_refPhysicsHitboxData`, two NM skeleton references, and parallel skeleton-related arrays of 94 entries. Its text metadata contains an AO-proxy capsule list, which is recorded only as embedded text and is not interpreted as gameplay hitbox geometry. The ignored `current-structural-analysis.json` preserves the structural facts and explicit no-promotion limits.

### Gate 1 acquisition audit — selected Anubis demo

The local-only acquisition report for `match730_003837681838084587988_1084783981_187.dem` independently records SHA-256 `e6fbdcd77267c6e38e9823fcaa4b37489e8e42b0894927aaf3e85c195ae363ae`, header patch `14175`, build `10847`, map `de_anubis`, and the completed Sentinel Source2 parser pass (68,484 ticks; 10 observed players). At tick 2, a real pawn/controller pair exposed `m_hModel=9371194797796759017`, `m_nHitboxSet=0`, controller `m_nPawnCharacterDefIndex=5308`, pose recipe version `2`, and a 64-byte AG2 payload with SHA-256 `b60d35c3e1bb5bc51fc0542994fb8118ca6c6a7d556801f0f65092d67e0e74a3`.

An independent protobuf-level audit and the full Source2 trace both found no `modelprecache`-style table, no modelprecache rows, no canonical player `.vmdl`/`.vmdl_c` path, and no resource-manifest binding for this observed handle. `AnimAssetData` disclosed only unrelated animation graph/skeleton resources. The resulting ignored `acquisition-report.json` therefore sets `model_resolution.available=false`, `canonical_vmdl_c_path=null`, `resource_sha256=null`, and `qualification.status=blocked_no_verified_model_binding`. No asset-name search, model resource extraction, `model-describe`, KV3 inspection, fixture qualification, fallback, AG2 decoder, or exact geometry operation ran from this audit.

### Gate 1A.5 model-handle resolution — `handle_not_resolvable_from_demo`

The next audit isolated the selected pawn’s `CBodyComponent.m_skeletonInstance.m_modelState.m_hModel = 9371194797796759017` through its early entity-property history. Its schema type is `CStrongHandle<InfoForResourceTypeCModel>`. Public CS2 schema metadata defines `CStrongHandle` as an `m_pBinding` pointer-like runtime binding, and `CModelState` separately contains an optional `m_ModelName` field. Those definitions establish resource type and runtime-binding shape, but they do not provide a portable handle-to-path algorithm. Sources: https://s2v.app/SchemaExplorer/cs2/_intrinsic/CStrongHandle, https://s2v.app/SchemaExplorer/cs2/server/CModelState and https://github.com/SteamTracking/GameTracking-CS2/blob/master/DumpSource2/schemas/server/CModelState.h.

For this demo, the property history contains `m_hModel` and model-state rendering fields but no `m_ModelName`, canonical resource path, binding object, resource manifest, GameResourceService, NetworkSystem record, `modelprecache` table or modelprecache row. The independent protobuf audit’s complete string-table catalog and decompressed path scan also contain no canonical player VMDL path. `AnimAssetData` only lists animation graph/skeleton resources and no observed relation to the handle. Therefore the ignored `handle-resolution-report.json` returns `handle_not_resolvable_from_demo`, not `parser_missing_required_table` or `handle_semantics_unknown`.

The permitted continuation changes accordingly: model identity must be supplied by an auditable build-matched external resource index or a runtime ResourceSystem capture binding this exact observed handle to a canonical compiled path. That record must be cross-checked against matching VMDL bytes and declared dependencies before a fixture, Gate 1D.0, Gate 1D.1, AG2, or exact geometry can proceed.
