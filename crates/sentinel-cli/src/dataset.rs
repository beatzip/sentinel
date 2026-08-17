use std::path::Path;

pub fn run(args: &[String]) {
    match args.first().map(String::as_str) {
        Some("init") => init(args.get(1).map(String::as_str).unwrap_or("datasets")),
        Some("audit") => audit(args.get(1).map(String::as_str)),
        _ => eprintln!("Usage: sentinel dataset <init [directory] | audit <manifest.json>>"),
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
