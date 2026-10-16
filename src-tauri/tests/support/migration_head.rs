//! Shared test helper: the latest migration version is derived from the
//! migration files themselves, so adding a migration does not require
//! editing every schema-head assertion.

/// Highest numeric prefix among `src-tauri/migrations/NNN_*.sql`.
pub fn latest_migration_version() -> i64 {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            if !name.ends_with(".sql") {
                return None;
            }
            let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<i64>().ok()
        })
        .max()
        .expect("at least one numbered migration")
}
