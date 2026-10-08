//! Bridge this fork's retired Alias schema v38 to upstream's schema v38.
//! Old bindings remain stored for recovery, but never become live mappings.

use super::{create_pre_version_backup, schema_version_on, table_exists, table_has_column};
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub(super) fn prepare_legacy_v38(conn: &Connection, db_path: &Path) -> Result<()> {
    if schema_version_on(conn)? != 38 || !table_exists(conn, "user_model_alias_bindings")? {
        return Ok(());
    }
    for column in ["alias", "provider_id", "upstream_model", "updated_at"] {
        anyhow::ensure!(
            table_has_column(conn, "user_model_alias_bindings", column)?,
            "legacy fork Alias table is missing {column}"
        );
    }
    let parents = table_exists(conn, "platform_accounts")?;
    let links = table_exists(conn, "platform_links")?;
    if parents && links {
        return Ok(());
    }
    anyhow::ensure!(
        !parents && !links,
        "incomplete upstream platform schema v38"
    );
    create_pre_version_backup(conn, db_path, "data.sqlite.pre-fork-v38.", 38)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE platform_accounts (
            id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN ('new_api','sub2api')),
            name TEXT NOT NULL, base_url TEXT NOT NULL, credential_cipher TEXT,
            version INTEGER NOT NULL DEFAULT 1, snapshot TEXT);
         CREATE TABLE platform_links (
            account_id TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
            platform_account_id TEXT NOT NULL REFERENCES platform_accounts(id) ON DELETE RESTRICT,
            group_json TEXT NOT NULL, version INTEGER NOT NULL DEFAULT 1, snapshot TEXT);",
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests;
