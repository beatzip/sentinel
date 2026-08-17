use crate::MatchReport;

/// HTML report generator
pub struct HtmlReport;

impl HtmlReport {
    /// Generate an HTML report from match data
    pub fn generate(report: &MatchReport) -> String {
        let mut html = String::new();

        // HTML header
        html.push_str(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sentinel AI Report</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f0f0f; color: #e0e0e0; padding: 20px; }
        .container { max-width: 1200px; margin: 0 auto; }
        h1 { color: #00ff88; margin-bottom: 20px; }
        h2 { color: #00ccff; margin: 30px 0 15px 0; border-bottom: 1px solid #333; padding-bottom: 10px; }
        h3 { color: #ffaa00; margin: 20px 0 10px 0; }
        .metadata { background: #1a1a1a; padding: 15px; border-radius: 8px; margin-bottom: 20px; }
        .metadata p { margin: 5px 0; }
        .score-badge { display: inline-block; padding: 5px 15px; border-radius: 20px; font-weight: bold; }
        .score-clean { background: #00ff8820; color: #00ff88; }
        .score-low { background: #ffff0020; color: #ffff00; }
        .score-moderate { background: #ffaa0020; color: #ffaa00; }
        .score-high { background: #ff550020; color: #ff5500; }
        .score-critical { background: #ff000020; color: #ff0000; }
        .player-card { background: #1a1a1a; padding: 20px; border-radius: 8px; margin-bottom: 15px; }
        .score-bar { display: inline-block; height: 20px; background: #333; border-radius: 4px; overflow: hidden; }
        .score-fill { height: 100%; background: linear-gradient(90deg, #00ff88, #ffaa00, #ff0000); }
        table { width: 100%; border-collapse: collapse; margin: 10px 0; }
        th, td { padding: 8px 12px; text-align: left; border-bottom: 1px solid #333; }
        th { color: #00ccff; }
        .footer { margin-top: 40px; color: #666; font-size: 12px; }
    </style>
</head>
<body>
    <div class="container">
"#);

        // Title
        html.push_str("        <h1>Sentinel AI Analysis Report</h1>\n");

        // Metadata
        html.push_str("        <div class=\"metadata\">\n");
        html.push_str(&format!(
            "            <p><strong>Map:</strong> {}</p>\n",
            report.metadata.map_name
        ));
        html.push_str(&format!(
            "            <p><strong>Server:</strong> {}</p>\n",
            report.metadata.server_name
        ));
        html.push_str(&format!(
            "            <p><strong>Rounds:</strong> {}</p>\n",
            report.metadata.total_rounds
        ));
        html.push_str(&format!(
            "            <p><strong>Duration:</strong> {:.1}s</p>\n",
            report.metadata.duration_seconds
        ));
        html.push_str(&format!(
            "            <p><strong>Demo:</strong> {}</p>\n",
            report.metadata.demo_path
        ));
        html.push_str("        </div>\n\n");

        // Overall score
        html.push_str("        <h2>Overall Analysis</h2>\n");
        let grade_class = Self::score_class(report.overall_anomaly);
        let grade_text = Self::score_grade(report.overall_anomaly);
        html.push_str(&format!(
            "        <p>Anomaly Score: <span class=\"score-badge {}\">{:.2}/1.0 - {}</span></p>\n",
            grade_class, report.overall_anomaly, grade_text
        ));

        // Player reports
        html.push_str("        <h2>Player Analysis</h2>\n");

        for player in &report.players {
            let player_grade = Self::score_class(player.scores.overall);

            html.push_str("        <div class=\"player-card\">\n");
            html.push_str(&format!(
                "            <h3>{} ({})</h3>\n",
                player.name, player.steam_id
            ));
            html.push_str(&format!(
                "            <p>Team: {} | Overall Score: <span class=\"score-badge {}\">{:.2}</span></p>\n",
                player.team, player_grade, player.scores.overall
            ));

            // Category scores
            html.push_str("            <table>\n");
            html.push_str(
                "                <tr><th>Category</th><th>Score</th><th>Visual</th></tr>\n",
            );
            for (category, score) in &player.scores.categories {
                let bar_width = (score * 100.0) as u32;
                html.push_str(&format!(
                    "                <tr><td>{}</td><td>{:.2}</td><td><div class=\"score-bar\"><div class=\"score-fill\" style=\"width: {}%\"></div></div></td></tr>\n",
                    category, score, bar_width
                ));
            }
            html.push_str("            </table>\n");

            // Evidence
            if !player.evidence.is_empty() {
                html.push_str(&format!(
                    "            <p><strong>Evidence:</strong> {} items</p>\n",
                    player.evidence.len()
                ));
                html.push_str("            <table>\n");
                html.push_str("                <tr><th>Tick</th><th>Feature</th><th>Score</th><th>Reason</th></tr>\n");

                for ev in &player.evidence {
                    html.push_str(&format!(
                        "                <tr><td>{}</td><td>{}</td><td>{:.2}</td><td>{}</td></tr>\n",
                        ev.tick.0, ev.feature, ev.score, ev.reason
                    ));
                }

                html.push_str("            </table>\n");
            }

            html.push_str("        </div>\n\n");
        }

        // Footer
        html.push_str(&format!(
            "        <div class=\"footer\">Report generated by Sentinel AI v{}</div>\n",
            report.version
        ));

        html.push_str("    </div>\n</body>\n</html>");

        html
    }

    fn score_class(score: f64) -> &'static str {
        if score < 0.2 {
            "score-clean"
        } else if score < 0.4 {
            "score-low"
        } else if score < 0.6 {
            "score-moderate"
        } else if score < 0.8 {
            "score-high"
        } else {
            "score-critical"
        }
    }

    fn score_grade(score: f64) -> &'static str {
        if score < 0.2 {
            "Clean"
        } else if score < 0.4 {
            "Low Suspicion"
        } else if score < 0.6 {
            "Moderate Suspicion"
        } else if score < 0.8 {
            "High Suspicion"
        } else {
            "Critical"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_generation() {
        use crate::{ConfidenceAssessment, MatchMetadata, MatchReport, PlayerReport};
        use sentinel_core::BehaviorScore;

        let metadata = MatchMetadata {
            demo_path: "test.dem".to_string(),
            map_name: "de_dust2".to_string(),
            server_name: "Test Server".to_string(),
            total_rounds: 30,
            duration_seconds: 1800.0,
            tick_rate: 64,
        };

        let mut report = MatchReport::new(metadata);
        report.add_player(PlayerReport {
            steam_id: 12345,
            name: "TestPlayer".to_string(),
            team: "Terrorist".to_string(),
            scores: BehaviorScore::new(),
            evidence: Vec::new(),
            summary: "No anomalies".to_string(),
            confidence: ConfidenceAssessment::default(),
        });

        let html = HtmlReport::generate(&report);
        assert!(html.contains("de_dust2"));
        assert!(html.contains("TestPlayer"));
        assert!(html.contains("<!DOCTYPE html>"));
    }
}
