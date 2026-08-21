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
