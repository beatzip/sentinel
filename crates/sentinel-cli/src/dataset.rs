use std::path::{Path, PathBuf};

use sentinel_analysis::{TemporalTransformer, XgBoostModel};

pub fn run(args: &[String]) {
    match args.first().map(String::as_str) {
        Some("init") => init(args.get(1).map(String::as_str).unwrap_or("datasets")),
        Some("audit") => audit(args.get(1).map(String::as_str)),
        Some("train") => train(
            args.get(1).map(String::as_str),
            args.get(2).map(String::as_str),
        ),
        Some("promote-reviews") => promote_reviews(
            args.get(1).map(String::as_str),
            args.get(2).map(String::as_str),
        ),
        _ => eprintln!(
            "Usage: sentinel dataset <init [directory] | audit <manifest.json> | train <manifest.json> [models_dir] | promote-reviews <manifest.json> <reviews.json>>"
        ),
    }
}

fn init(directory: &str) {
    let root = Path::new(directory);
    for dir in ["legit/hltv", "legit/faceit", "cheater", "unknown"] {
        if let Err(error) = std::fs::create_dir_all(root.join(dir)) {
            eprintln!("Error creating dataset layout: {error}");
            return;
        }
    }

    let manifest = root.join("manifest.json");
    if manifest.exists() {
        eprintln!("Manifest already exists: {}", manifest.display());
        return;
    }
    if let Err(error) = sentinel_datasets::DatasetManifest::default().save(&manifest) {
        eprintln!("Error writing manifest: {error}");
        return;
    }
    println!("Dataset layout created at {}", root.display());
}

fn audit(manifest_path: Option<&str>) {
    let Some(manifest_path) = manifest_path else {
        eprintln!("Usage: sentinel dataset audit <manifest.json>");
        return;
    };
    let path = Path::new(manifest_path);
    let manifest = match sentinel_datasets::DatasetManifest::load(path) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("Error loading manifest: {error}");
            return;
        }
    };
    let audit = manifest.audit(path.parent().unwrap_or_else(|| Path::new(".")));
    println!("Dataset audit:");
    println!("  Demos: {}", audit.total);
    println!(
        "  Labels — legit: {}, cheater: {}, unknown: {}",
        audit.legit, audit.cheater, audit.unknown
    );
    println!("  Missing files: {}", audit.missing_files);
    println!("  Unverified labels: {}", audit.unverified);
    println!("  Duplicate paths: {}", audit.duplicate_paths);
}

fn promote_reviews(manifest_path: Option<&str>, reviews_path: Option<&str>) {
    let (Some(manifest_path), Some(reviews_path)) = (manifest_path, reviews_path) else {
        eprintln!("Usage: sentinel dataset promote-reviews <manifest.json> <reviews.json>");
        return;
    };
    let manifest_path = Path::new(manifest_path);
    let mut manifest = match sentinel_datasets::DatasetManifest::load(manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("Error loading manifest: {error}");
            return;
        }
    };
    let reviews = match sentinel_datasets::ReviewManifest::load(Path::new(reviews_path)) {
        Ok(reviews) => reviews,
        Err(error) => {
            eprintln!("Error loading reviews: {error}");
            return;
        }
    };
    let promotion = manifest.apply_verified_reviews(&reviews);
    if let Err(error) = manifest.save(manifest_path) {
        eprintln!("Error saving manifest: {error}");
        return;
    }
    println!("Review promotion:");
    println!("  Promoted: {}", promotion.promoted);
    println!("  Unverified: {}", promotion.skipped_unverified);
    println!("  Ambiguous: {}", promotion.skipped_ambiguous);
    println!("  Missing evidence: {}", promotion.skipped_missing_evidence);
    println!(
        "  Missing manifest entry: {}",
        promotion.skipped_missing_entry
    );
}

fn train(manifest_path: Option<&str>, models_dir: Option<&str>) {
    let Some(manifest_path) = manifest_path else {
        eprintln!("Usage: sentinel dataset train <manifest.json> [models_dir]");
        return;
    };
    let manifest_path = Path::new(manifest_path);
    let manifest = match sentinel_datasets::DatasetManifest::load(manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("Error loading manifest: {error}");
            return;
        }
    };
    let root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let corpus = match manifest.supervised_corpus(root) {
        Ok(corpus) => corpus,
        Err(error) => {
            eprintln!("Training corpus rejected: {error}");
            return;
        }
    };
    let output = PathBuf::from(models_dir.unwrap_or("models"));
    if let Err(error) = std::fs::create_dir_all(&output) {
        eprintln!("Error creating model directory: {error}");
        return;
    }
    let mut xgboost = XgBoostModel::new();
    if let Err(error) = xgboost.train_labeled(&corpus.vectors) {
        eprintln!("XGBoost training failed: {error}");
        return;
    }
    if let Err(error) = xgboost.save(&output.join("sentinel-xgboost.sqb")) {
        eprintln!("Error saving XGBoost model: {error}");
        return;
    }
    let mut transformer = TemporalTransformer::default();
    if let Err(error) = transformer.train_labeled(&corpus.sequences) {
        eprintln!("Transformer training failed: {error}");
        return;
    }
    let transformer_json = match serde_json::to_string_pretty(&transformer) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Error serializing Transformer: {error}");
            return;
        }
    };
    if let Err(error) = std::fs::write(output.join("sentinel-transformer.json"), transformer_json) {
        eprintln!("Error saving Transformer: {error}");
        return;
    }
    let metadata = serde_json::json!({
        "schema_version": 1,
        "vector_count": corpus.vectors.len(),
        "sequence_count": corpus.sequences.len(),
        "xgboost_features": xgboost.feature_names(),
    });
    if let Err(error) = std::fs::write(
        output.join("training-metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    ) {
        eprintln!("Error saving training metadata: {error}");
        return;
    }
    println!(
        "Trained {} vectors and {} sequences into {}",
        corpus.vectors.len(),
        corpus.sequences.len(),
        output.display()
    );
}
