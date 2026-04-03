// Fin + Database Schema & Migration

#[allow(dead_code)]
/// Current schema version. Bump this when adding migrations.
pub const SCHEMA_VERSION: u32 = 1;

#[allow(dead_code)]
/// Check and run pending migrations.
pub fn migrate(conn: &rusqlite::Connection, current: u32, target: u32) -> anyhow::Result<()> {
    for version in current..target {
        match version {
            0 => {
                // Initial schema created in ProjectDb::migrate()
                tracing::info!("Schema v1 initialized");
            }
            _ => {
                tracing::warn!("Unknown schema version: {version}");
            }
        }
    }

    conn.execute(
        "INSERT OR REPLACE INTO schema_version (rowid, version) VALUES (1, ?)",
        [target],
    )?;

    Ok(())
}
