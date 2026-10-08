use super::*;

#[test]
fn unrelated_schema_v38_is_untouched() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE schema_version(version INTEGER PRIMARY KEY); INSERT INTO schema_version VALUES(38);").unwrap();
    prepare_legacy_v38(&conn, Path::new("unused.sqlite")).unwrap();
    assert!(!table_exists(&conn, "platform_accounts").unwrap());
    assert_eq!(schema_version_on(&conn).unwrap(), 38);
}

#[test]
fn incomplete_platform_schema_is_rejected_before_writes() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE schema_version(version INTEGER PRIMARY KEY); INSERT INTO schema_version VALUES(38);
        CREATE TABLE user_model_alias_bindings(alias TEXT, provider_id TEXT, upstream_model TEXT, updated_at TEXT);
        CREATE TABLE platform_accounts(id TEXT);").unwrap();
    assert!(prepare_legacy_v38(&conn, Path::new("unused.sqlite")).is_err());
    assert!(!table_exists(&conn, "platform_links").unwrap());
    assert_eq!(schema_version_on(&conn).unwrap(), 38);
}
