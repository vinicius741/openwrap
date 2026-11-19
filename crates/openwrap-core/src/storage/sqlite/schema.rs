use rusqlite::Connection;

use crate::errors::AppError;

/// Adds `column` to `table` when it is missing. Returns `true` when the column
/// was added by this call, so callers can run one-time data migrations.
pub fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<bool, AppError> {
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

    Ok(!exists)
}
