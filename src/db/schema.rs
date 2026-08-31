//! 幂等 schema：只用 TEXT/INTEGER 可移植类型，SQLite/PG 通用

use super::Db;

/// 在启动时执行 CREATE TABLE IF NOT EXISTS
pub async fn ensure_schema(db: &Db) -> anyhow::Result<()> {
    // secrets 主表
    db.execute(
        "CREATE TABLE IF NOT EXISTS secrets (
            id TEXT PRIMARY KEY,
            project TEXT NOT NULL,
            service TEXT NOT NULL DEFAULT '',
            key_name TEXT NOT NULL,
            value_enc TEXT NOT NULL,
            crypto_version INTEGER NOT NULL DEFAULT 1,
            description TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '[]',
            source TEXT NOT NULL DEFAULT '',
            identity TEXT NOT NULL DEFAULT '',
            probe_data TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE (project, service, key_name)
        )",
    )
    .await?;
    // 老库升级：补 identity / probe_data 列（幂等）
    let _ = db
        .execute("ALTER TABLE secrets ADD COLUMN identity TEXT NOT NULL DEFAULT ''")
        .await;
    let _ = db
        .execute("ALTER TABLE secrets ADD COLUMN probe_data TEXT NOT NULL DEFAULT '{}'")
        .await;

    db.execute("CREATE INDEX IF NOT EXISTS idx_secrets_project ON secrets (project)")
        .await?;
    db.execute("CREATE INDEX IF NOT EXISTS idx_secrets_key_name ON secrets (key_name)")
        .await?;

    // 分组
    db.execute(
        "CREATE TABLE IF NOT EXISTS collections (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        )",
    )
    .await?;

    db.execute(
        "CREATE TABLE IF NOT EXISTS collection_items (
            collection_id TEXT NOT NULL,
            secret_id TEXT NOT NULL,
            PRIMARY KEY (collection_id, secret_id)
        )",
    )
    .await?;

    // API keys（存 hash）
    db.execute(
        "CREATE TABLE IF NOT EXISTS api_keys (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL UNIQUE,
            scopes TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            last_used_at TEXT
        )",
    )
    .await?;

    // meta（bootstrap 状态、admin hash、schema_version）
    db.execute(
        "CREATE TABLE IF NOT EXISTS meta (
            k TEXT PRIMARY KEY,
            v TEXT NOT NULL
        )",
    )
    .await?;

    // 审计日志
    db.execute(
        "CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            actor TEXT NOT NULL DEFAULT '',
            action TEXT NOT NULL,
            secret_id TEXT,
            at TEXT NOT NULL,
            ip TEXT NOT NULL DEFAULT ''
        )",
    )
    .await?;

    Ok(())
}
