use std::path::{Path, PathBuf};

use axum::{
    Json, Router,
    extract::{Path as ApiPath, State},
    http::StatusCode,
    routing::get,
};
use sentinel_report::{
    AnalysisProvenance, ConfidenceAssessment, MatchReport, PlayerReport, ReanalysisStatus,
    SupportingMatch, replay::ReplayData,
};
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct ApiState {
    reports_dir: PathBuf,
}

#[derive(Serialize)]
struct Health {
    service: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct ReportSummary {
    id: String,
    map: String,
    rounds: u32,
    anomaly_score: f64,
}

#[derive(Serialize)]
struct PlayerHistory {
    report_id: String,
    map: String,
    player: PlayerReport,
}

#[derive(Serialize)]
struct DossierMatch {
    report_id: String,
    map: String,
    player: PlayerReport,
    provenance: AnalysisProvenance,
    reanalysis: ReanalysisStatus,
}

/// Profile data computed exclusively from locally published Sentinel reports.
#[derive(Serialize)]
struct PlayerDossier {
    steam_id: u64,
    name: String,
    matches_observed: usize,
    flagged_matches: usize,
    confidence: ConfidenceAssessment,
    matches: Vec<DossierMatch>,
}

#[derive(Serialize)]
struct OverlaySnapshot {
    report_id: String,
    map: String,
    overall_anomaly: f64,
    players: Vec<OverlayPlayer>,
}

#[derive(Serialize)]
struct OverlayPlayer {
    name: String,
    anomaly_score: f64,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;

#[tokio::main]
async fn main() {
    let reports_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("reports"));
    let address =
        std::env::var("SENTINEL_API_BIND").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| panic!("Unable to bind {address}: {error}"));
    println!("Sentinel API listening on http://{address}");
    println!("Reading reports from {}", reports_dir.display());
    axum::serve(listener, app(reports_dir)).await.unwrap();
}

fn app(reports_dir: PathBuf) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/reports", get(list_reports))
        .route("/v1/reports/{id}", get(get_report))
        .route("/v1/replays/{id}", get(get_replay))
        .route("/v1/players/{steam_id}", get(player_history))
        .route("/v1/players/{steam_id}/dossier", get(player_dossier))
        .route("/v1/overlay/{id}", get(overlay_snapshot))
        .with_state(ApiState { reports_dir })
        .layer(CorsLayer::new().allow_origin(Any))
}

async fn health() -> Json<Health> {
    Json(Health {
        service: "sentinel-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn list_reports(State(state): State<ApiState>) -> ApiResult<Vec<ReportSummary>> {
    let reports = load_reports(&state.reports_dir)?;
    Ok(Json(
        reports
            .into_iter()
            .map(|(id, report)| ReportSummary {
                id,
                map: report.metadata.map_name,
                rounds: report.metadata.total_rounds,
                anomaly_score: report.overall_anomaly,
            })
            .collect(),
    ))
}

async fn get_report(
    State(state): State<ApiState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<MatchReport> {
    let path = report_path(&state.reports_dir, &id)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid report id".to_string()))?;
    match read_report(&path) {
        Ok(report) => Ok(Json(report)),
        Err(_) => Err((StatusCode::NOT_FOUND, "Report not found".to_string())),
    }
}

async fn get_replay(
    State(state): State<ApiState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<ReplayData> {
    let path = replay_path(&state.reports_dir, &id)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid replay id".to_string()))?;
    match read_replay(&path) {
        Ok(replay) => Ok(Json(replay)),
        Err(_) => Err((StatusCode::NOT_FOUND, "Replay not found".to_string())),
    }
}

async fn player_history(
    State(state): State<ApiState>,
    ApiPath(steam_id): ApiPath<u64>,
) -> ApiResult<Vec<PlayerHistory>> {
    let reports = load_reports(&state.reports_dir)?;
    let history = reports
        .into_iter()
        .flat_map(|(report_id, report)| {
            let map = report.metadata.map_name;
            report
                .players
                .into_iter()
                .filter(move |player| player.steam_id == steam_id)
                .map(move |player| PlayerHistory {
                    report_id: report_id.clone(),
                    map: map.clone(),
                    player,
                })
        })
        .collect();
    Ok(Json(history))
}

async fn player_dossier(
    State(state): State<ApiState>,
    ApiPath(steam_id): ApiPath<u64>,
) -> ApiResult<PlayerDossier> {
    let reports = load_reports(&state.reports_dir)?;
    let mut matches = reports
        .into_iter()
        .flat_map(|(report_id, report)| {
            let map = report.metadata.map_name;
            let provenance = report.provenance;
            let reanalysis = report.reanalysis;
            report
                .players
                .into_iter()
                .filter(move |player| player.steam_id == steam_id)
                .map(move |player| DossierMatch {
                    report_id: report_id.clone(),
                    map: map.clone(),
                    player,
                    provenance: provenance.clone(),
                    reanalysis: reanalysis.clone(),
                })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.report_id.cmp(&right.report_id));
    let current = matches
        .iter()
        .max_by(|left, right| {
            left.player
                .scores
                .overall
                .total_cmp(&right.player.scores.overall)
        })
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "Player not found in local reports".to_string(),
            )
        })?;
    let flagged_matches = matches
        .iter()
        .filter(|entry| entry.player.scores.overall >= 0.6)
        .count();
    let supporting_matches = matches
        .iter()
        .filter(|entry| entry.player.scores.overall >= 0.5)
        .map(|entry| SupportingMatch {
            report_id: entry.report_id.clone(),
            map_name: entry.map.clone(),
            overall_score: entry.player.scores.overall,
            evidence_count: entry.player.evidence.len(),
            flagged: entry.player.scores.overall >= 0.6,
        })
        .collect();
    let confidence = ConfidenceAssessment::assess(
        &current.player.scores,
        matches.len(),
        flagged_matches,
        supporting_matches,
    );
    Ok(Json(PlayerDossier {
        steam_id,
        name: current.player.name.clone(),
        matches_observed: matches.len(),
        flagged_matches,
        confidence,
        matches,
    }))
}

async fn overlay_snapshot(
    State(state): State<ApiState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<OverlaySnapshot> {
    let path = report_path(&state.reports_dir, &id)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid report id".to_string()))?;
    let report =
        read_report(&path).map_err(|_| (StatusCode::NOT_FOUND, "Report not found".to_string()))?;
    let mut players = report
        .players
        .into_iter()
        .map(|player| OverlayPlayer {
            name: player.name,
            anomaly_score: player.scores.overall,
        })
        .collect::<Vec<_>>();
    players.sort_by(|left, right| right.anomaly_score.total_cmp(&left.anomaly_score));
    Ok(Json(OverlaySnapshot {
        report_id: id,
        map: report.metadata.map_name,
        overall_anomaly: report.overall_anomaly,
        players,
    }))
}

fn report_path(root: &Path, id: &str) -> Option<PathBuf> {
    (!id.is_empty()
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        }))
    .then(|| root.join(format!("{id}.json")))
}

fn replay_path(root: &Path, id: &str) -> Option<PathBuf> {
    (!id.is_empty()
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        }))
    .then(|| root.join(format!("{id}.replay.json")))
}

fn load_reports(root: &Path) -> Result<Vec<(String, MatchReport)>, (StatusCode, String)> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err((StatusCode::INTERNAL_SERVER_ERROR, error.to_string())),
    };
    Ok(entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                return None;
            }
            Some((
                path.file_stem()?.to_str()?.to_string(),
                read_report(&path).ok()?,
            ))
        })
        .collect())
}

fn read_report(path: &Path) -> Result<MatchReport, std::io::Error> {
    let json = std::fs::read_to_string(path)?;
    serde_json::from_str(&json).map_err(std::io::Error::other)
}

fn read_replay(path: &Path) -> Result<ReplayData, std::io::Error> {
    let json = std::fs::read_to_string(path)?;
    serde_json::from_str(&json).map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_id_cannot_escape_the_report_directory() {
        assert!(report_path(Path::new("reports"), "match_01").is_some());
        assert!(report_path(Path::new("reports"), "../secrets").is_none());
        assert!(replay_path(Path::new("reports"), "match_01").is_some());
    }
}
