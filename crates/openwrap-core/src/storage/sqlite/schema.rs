//! Database schema and migration logic.

use rusqlite::Connection;

use crate::errors::AppError;

/// Runs all database migrations.
pub fn migrate(connection: &Connection) -> Result<(), AppError> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            source_filename TEXT NOT NULL,
            managed_dir TEXT NOT NULL,
            managed_ovpn_path TEXT NOT NULL,
            original_import_path TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            dns_intent_json TEXT NOT NULL,
            dns_policy TEXT NOT NULL DEFAULT 'SplitDnsPreferred',
            credential_mode TEXT NOT NULL,
            remote_summary TEXT NOT NULL,
            has_saved_credentials INTEGER NOT NULL DEFAULT 0,
            validation_status TEXT NOT NULL,
            last_used_at TEXT NULL
        );

        CREATE TABLE IF NOT EXISTS profile_assets (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            sha256 TEXT NOT NULL,
            origin TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS profile_validation_findings (
            profile_id TEXT NOT NULL,
            directive TEXT NOT NULL,
            line INTEGER NOT NULL,
            severity TEXT NOT NULL,
            message TEXT NOT NULL,
            action TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS connection_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id TEXT NOT NULL,
            connected_at TEXT NOT NULL,
            state TEXT NOT NULL
        );
        "#,
    )?;
    ensure_column(
        connection,
        "profiles",
        "dns_policy",
        "TEXT NOT NULL DEFAULT 'SplitDnsPreferred'",
    )?;
    Ok(())
}

/// Ensures a column exists in a table, adding it if necessary.
fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), AppError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .any(|current| current == column);

    if !exists {
        connection.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }

    Ok(())
}
