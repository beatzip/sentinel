use crate::{SkeletonMetadata, Vec3};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Provenance for resolved geometry. Only `ExactDemo` may later participate in
/// exact-hitbox evidence after asset, pose and tick validation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HitboxGeometrySource {
    ExactDemo,
    GenericFallback,
    Unresolved,
}

/// Deliberately categorical: this is not a score and cannot imply precision
/// that the input telemetry does not provide.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HitboxGeometryConfidence {
    Exact,
    Approximate,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedHitboxCapsule {
    pub name: String,
    pub group_id: i32,
    pub radius: f32,
    pub start: Vec3,
    pub end: Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedHitboxGeometry {
    pub source: HitboxGeometrySource,
    pub confidence: HitboxGeometryConfidence,
    pub profile: Option<String>,
    pub model_handle: Option<u64>,
    pub hitbox_set: Option<u8>,
    pub observed_duck_amount: Option<f32>,
    pub capsules: Vec<ResolvedHitboxCapsule>,
}

#[derive(Debug, Deserialize)]
struct GenericProfile {
    profile: String,
    hitboxes: Vec<GenericHitbox>,
    crouch_modifier: CrouchModifier,
}

#[derive(Debug, Deserialize)]
struct GenericHitbox {
    name: String,
    bone: String,
    #[serde(rename = "groupId")]
    group_id: i32,
    radius: f32,
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Debug, Deserialize)]
struct CrouchModifier {
    spine_offset_z: f32,
    head_offset_z: f32,
    leg_scale: f32,
}

fn generic_profile() -> &'static GenericProfile {
    static PROFILE: OnceLock<GenericProfile> = OnceLock::new();
    PROFILE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../assets/standard_player_hitboxes.user_supplied.json"
        ))
        .expect("embedded generic hitbox profile must be valid JSON")
    })
}

/// Returns an explicitly approximate player-shaped fallback from the supplied
/// body-local profile. It is functional geometry, not model, bone, or hitbox proof.
pub fn resolve_standard_player_fallback(
    origin: Vec3,
    yaw_degrees: f32,
    skeleton: &SkeletonMetadata,
) -> ResolvedHitboxGeometry {
    let profile = generic_profile();
    let duck_amount = skeleton
        .duck_amount
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0));
    let duck = duck_amount.unwrap_or(0.0);
    let yaw = yaw_degrees.to_radians();
    let (sin, cos) = yaw.sin_cos();
    let capsules = profile
        .hitboxes
        .iter()
        .map(|hitbox| {
            let (mut start, mut end, mut radius) = (
                Vec3::new(hitbox.min[0], hitbox.min[1], hitbox.min[2]),
                Vec3::new(hitbox.max[0], hitbox.max[1], hitbox.max[2]),
                hitbox.radius,
            );

            // ponytail: the supplied fallback has no verified bone matrices. Keep this
            // as one bounded heuristic; replace it only with AG2/model-derived transforms.
            if hitbox.bone.starts_with("leg_") || hitbox.bone.starts_with("ankle_") {
                let scale = 1.0 + (profile.crouch_modifier.leg_scale - 1.0) * duck;
                start = scale_vec3(start, scale);
                end = scale_vec3(end, scale);
                radius *= scale;
            } else if hitbox.bone == "head_0" || hitbox.bone == "neck_0" {
                let offset = profile.crouch_modifier.head_offset_z * duck;
                start.z += offset;
                end.z += offset;
            } else if hitbox.bone == "pelvis" || hitbox.bone.starts_with("spine_") {
                let offset = profile.crouch_modifier.spine_offset_z * duck;
                start.z += offset;
                end.z += offset;
            }

            ResolvedHitboxCapsule {
                name: hitbox.name.clone(),
                group_id: hitbox.group_id,
                radius,
                start: rotate_then_translate(start, origin, sin, cos),
                end: rotate_then_translate(end, origin, sin, cos),
            }
        })
        .collect();

    ResolvedHitboxGeometry {
        source: HitboxGeometrySource::GenericFallback,
        confidence: HitboxGeometryConfidence::Approximate,
        profile: Some(profile.profile.clone()),
        model_handle: skeleton.model_handle,
        hitbox_set: skeleton.hitbox_set,
        observed_duck_amount: duck_amount,
        capsules,
    }
}

fn rotate_then_translate(local: Vec3, origin: Vec3, sin: f32, cos: f32) -> Vec3 {
    Vec3::new(
        origin.x + local.x * cos - local.y * sin,
        origin.y + local.x * sin + local.y * cos,
        origin.z + local.z,
    )
}

fn scale_vec3(value: Vec3, scale: f32) -> Vec3 {
    Vec3::new(value.x * scale, value.y * scale, value.z * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_fallback_never_claims_exact_geometry() {
        let geometry = resolve_standard_player_fallback(
            Vec3::new(10.0, 20.0, 30.0),
            0.0,
            &SkeletonMetadata::default(),
        );
        assert_eq!(geometry.source, HitboxGeometrySource::GenericFallback);
        assert_eq!(geometry.confidence, HitboxGeometryConfidence::Approximate);
        assert_eq!(geometry.capsules.len(), 19);
        assert!(geometry.profile.is_some());
    }

    #[test]
    fn observed_duck_amount_changes_generic_upper_body_geometry() {
        let standing =
            resolve_standard_player_fallback(Vec3::default(), 0.0, &SkeletonMetadata::default());
        let crouching = resolve_standard_player_fallback(
            Vec3::default(),
            0.0,
            &SkeletonMetadata {
                duck_amount: Some(1.0),
                ..SkeletonMetadata::default()
            },
        );
        let standing_head = standing
            .capsules
            .iter()
            .find(|capsule| capsule.name == "head")
            .expect("profile has head");
        let crouching_head = crouching
            .capsules
            .iter()
            .find(|capsule| capsule.name == "head")
            .expect("profile has head");
        assert!(crouching_head.start.z < standing_head.start.z);
    }
}
