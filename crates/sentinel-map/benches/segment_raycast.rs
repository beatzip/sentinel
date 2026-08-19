use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use sentinel_map::{MapData, Material, Vec2, Vec3, WallSegment};
use std::path::PathBuf;

fn synthetic_wall() -> MapData {
    let mut map = MapData {
        name: "synthetic_wall".to_owned(),
        bounds: (-100.0, -100.0, 100.0, 100.0),
        walls: vec![WallSegment {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(100.0, 0.0),
            material: Material::Solid,
        }],
        spawns: Vec::new(),
        bombsites: Vec::new(),
        nav_nodes: Vec::new(),
        bvh: None,
    };
    map.build_bvh_from_walls();
    map
}

fn anubis_map() -> MapData {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tris/de_anubis.tri");
    MapData::load_from_tri(&path).expect("de_anubis.tri must be present for this benchmark")
}

fn anubis_rays() -> [(Vec3, Vec3); 5] {
    [
        (
            Vec3::new(-458.0, 2160.0, 50.0),
            Vec3::new(-300.0, 2100.0, 50.0),
        ),
        (
            Vec3::new(-900.0, 1600.0, 50.0),
            Vec3::new(-250.0, 2050.0, 50.0),
        ),
        (
            Vec3::new(200.0, 800.0, 50.0),
            Vec3::new(1200.0, 1700.0, 50.0),
        ),
        (
            Vec3::new(-1600.0, 2800.0, 50.0),
            Vec3::new(-600.0, 1800.0, 50.0),
        ),
        (
            Vec3::new(1000.0, 3100.0, 50.0),
            Vec3::new(2000.0, 3900.0, 50.0),
        ),
    ]
}

fn bench_segment_raycast(c: &mut Criterion) {
    let mut group = c.benchmark_group("segment_blocked_3d");
    let wall = synthetic_wall();
    let observer = Vec3::new(50.0, -50.0, 50.0);

    group.bench_function("synthetic_clear_before_wall", |b| {
        let target = Vec3::new(50.0, -10.0, 50.0);
        b.iter(|| black_box(wall.segment_blocked_3d(black_box(observer), black_box(target))));
    });
    group.bench_function("synthetic_blocked_through_wall", |b| {
        let target = Vec3::new(50.0, 50.0, 50.0);
        b.iter(|| black_box(wall.segment_blocked_3d(black_box(observer), black_box(target))));
    });

    let anubis = anubis_map();
    let rays = anubis_rays();
    group.bench_with_input(
        BenchmarkId::new("de_anubis", "single_segment"),
        &rays[0],
        |b, ray| {
            b.iter(|| black_box(anubis.segment_blocked_3d(black_box(ray.0), black_box(ray.1))));
        },
    );
    group.bench_with_input(
        BenchmarkId::new("de_anubis", "45_fixed_segments"),
        &rays,
        |b, rays| {
            b.iter(|| {
                for index in 0..45 {
                    let (from, to) = rays[index % rays.len()];
                    black_box(anubis.segment_blocked_3d(black_box(from), black_box(to)));
                }
            });
        },
    );
    group.finish();
}

criterion_group!(benches, bench_segment_raycast);
criterion_main!(benches);
