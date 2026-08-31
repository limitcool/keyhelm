//! 数据库层：SQLite / PostgreSQL 双后端（sqlx runtime 查询）

pub mod repo;
pub mod schema;

use std::str::FromStr;

use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Postgres, Sqlite};

use crate::config::{DbConfig, DbKind};

/// 统一句柄：两种后端之一
#[derive(Clone)]
pub enum Db {
    Sqlite(Pool<Sqlite>),
    Postgres(Pool<Postgres>),
}

impl Db {
    /// 执行一条无占位符的 SQL（schema 用）
    pub async fn execute(&self, sql: &str) -> anyhow::Result<u64> {
        match self {
            Db::Sqlite(p) => Ok(sqlx::query(sql).execute(p).await?.rows_affected()),
            Db::Postgres(p) => Ok(sqlx::query(sql).execute(p).await?.rows_affected()),
        }
    }

    /// 依据配置构建连接池并执行幂等 schema
    pub async fn connect(cfg: &DbConfig) -> anyhow::Result<Self> {
        let db = match cfg.kind {
            DbKind::Sqlite => {
                use sqlx::sqlite::SqliteConnectOptions;
                let path = cfg.sqlite_path.display().to_string();
                if path != ":memory:" {
                    if let Some(parent) = cfg.sqlite_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                }
                // 用 SqliteConnectOptions::filename 构造，避免 URL 解析在 Windows 上的路径问题
                let mut opts = if path == ":memory:" {
                    SqliteConnectOptions::from_str("sqlite::memory:")?
                } else {
                    SqliteConnectOptions::new().filename(&cfg.sqlite_path)
                };
                opts = opts.create_if_missing(true);
                let pool = SqlitePoolOptions::new()
                    .max_connections(cfg.pool_max_conns)
                    .connect_with(opts)
                    .await?;
                if path != ":memory:" {
                    let _ = sqlx::query("PRAGMA journal_mode = WAL")
                        .execute(&pool)
                        .await;
                }
                Db::Sqlite(pool)
            }
            DbKind::Postgres => {
                let pool = PgPoolOptions::new()
                    .max_connections(cfg.pool_max_conns)
                    .connect(&cfg.postgres_url)
                    .await?;
                Db::Postgres(pool)
            }
        };
        schema::ensure_schema(&db).await?;
        Ok(db)
    }
}

/// 供测试用的内存 sqlite
#[cfg(test)]
pub async fn test_db() -> Db {
    let opts = sqlx::sqlite::SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    let db = Db::Sqlite(pool);
    schema::ensure_schema(&db).await.unwrap();
    db
}
