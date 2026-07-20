use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sentinel_core::{Angles, PlayerId, PlayerState, Tick, TickState, Vec3, Weapon};
use sentinel_core::bomb::BombState;
use sentinel_core::round::RoundState;
use sentinel_features::FeatureEngine;
use sentinel_analysis::Scorer;
use sentinel_visibility::VisibilityEngine;

fn create_test_state(num_players: usize) -> TickState {
    let mut players = Vec::new();

    for i in 0..num_players {
        let team = if i % 2 == 0 { sentinel_core::Team::Terrorist } else { sentinel_core::Team::CounterTerrorist };
        let x = (i as f32) * 500.0;

        players.push(PlayerState {
            id: PlayerId::new(i as u64 + 1),
            name: format!("Player_{}", i),
            team,
            position: Vec3::new(x, 0.0, 0.0),
            velocity: Vec3::default(),
            view_angles: Angles { pitch: 0.0, yaw: 0.0, roll: 0.0 },
            weapon: Weapon::Rifle,
            health: 100,
            armor: 100,
            money: 4500,
            flash_duration: 0.0,
            scoped: false,
            reloading: false,
            alive: true,
        });
    }

    TickState {
        tick: Tick(1000),
        players,
        grenades: Vec::new(),
        bomb: BombState::Carried { carrier: PlayerId::new(0) },
        round: RoundState {
            round_number: 1,
            phase: sentinel_core::RoundPhase::Live,
            clock: 100.0,
            t_score: 0,
            ct_score: 0,
            winner: None,
        },
    }
}

fn bench_feature_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_engine");

    for num_players in [5, 10, 20] {
        let state = create_test_state(num_players);
        let ctx = sentinel_core::MatchContext::new(vec![state]);
        let engine = FeatureEngine::new();
        let players: Vec<PlayerId> = (1..=num_players as u64).map(PlayerId::new).collect();

        group.bench_with_input(
            BenchmarkId::new("compute_match", format!("{}players", num_players)),
            &players,
            |b, players| {
                b.iter(|| {
                    for &player in players {
                        black_box(engine.compute_match(&ctx, player));
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_visibility(c: &mut Criterion) {
    let state = create_test_state(10);
    let players: Vec<PlayerId> = (1..=10).map(PlayerId::new).collect();

    c.bench_function("visibility_can_see", |b| {
        b.iter(|| {
            for &observer in &players {
                for &target in &players {
                    if observer != target {
                        black_box(VisibilityEngine::can_see(&state, observer, target));
                    }
                }
            }
        });
    });

    c.bench_function("visibility_can_hear", |b| {
        b.iter(|| {
            for &observer in &players {
                for &target in &players {
                    if observer != target {
                        black_box(VisibilityEngine::can_hear(&state, observer, target));
                    }
                }
            }
        });
    });
}

fn bench_scorer(c: &mut Criterion) {
    let mut group = c.benchmark_group("scorer");

    for num_features in [5, 10, 17] {
        let state = create_test_state(10);
        let ctx = sentinel_core::MatchContext::new(vec![state]);
        let engine = FeatureEngine::new();
        let scorer = Scorer::default_cs2();

        // Pre-compute feature vectors
        let players: Vec<PlayerId> = (1..=10).map(PlayerId::new).collect();
        let mut all_fvs = Vec::new();
        for &player in &players {
            let fv = engine.compute_all(&ctx, Tick(1000), player);
            all_fvs.push(fv);
        }

        group.bench_with_input(
            BenchmarkId::new("score_player", format!("{}features", num_features)),
            &players,
            |b, players| {
                b.iter(|| {
                    for &player in players {
                        let fvs: Vec<&FeatureVector> = all_fvs.iter().filter(|fv| fv.player == player).collect();
                        black_box(scorer.score_player(player, &fvs));
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_anomaly_score(c: &mut Criterion) {
    let mut group = c.benchmark_group("anomaly_score");

    let baseline = sentinel_analysis::FeatureBaseline::new("test", 100.0, 10.0);

    group.bench_function("z_score", |b| {
        b.iter(|| black_box(baseline.z_score(black_box(115.0))));
    });

    group.bench_function("anomaly_score", |b| {
        b.iter(|| black_box(baseline.anomaly_score(black_box(115.0))));
    });

    group.finish();
}

fn bench_bayesian(c: &mut Criterion) {
    let mut group = c.benchmark_group("bayesian");

    group.bench_function("combine_5_scores", |b| {
        let scores = vec![0.7, 0.8, 0.6, 0.9, 0.5];
        b.iter(|| black_box(sentinel_analysis::BayesianAggregator::combine_scores(&scores)));
    });

    group.bench_function("combine_10_scores", |b| {
        let scores = vec![0.7, 0.8, 0.6, 0.9, 0.5, 0.4, 0.85, 0.75, 0.65, 0.55];
        b.iter(|| black_box(sentinel_analysis::BayesianAggregator::combine_scores(&scores)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_feature_engine,
    bench_visibility,
    bench_scorer,
    bench_anomaly_score,
    bench_bayesian
);
criterion_main!(benches);
