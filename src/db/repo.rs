//! Repo 层：所有 SQL 用 runtime query + `?` 占位符，双后端通用

use sqlx::postgres::PgRow;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::model::{
    ApiKey, Collection, CreateSecretRequest, ProjectNode, Secret, SecretMeta, UpdateSecretRequest,
};

use super::Db;

/// 把 sqlx 行转换为 Secret（双后端各自实现）
trait RowSecret {
    fn to_secret(&self) -> anyhow::Result<Secret>;
}

impl RowSecret for SqliteRow {
    fn to_secret(&self) -> anyhow::Result<Secret> {
        Ok(Secret {
            id: self.try_get("id")?,
            project: self.try_get("project")?,
            service: self.try_get("service")?,
            key_name: self.try_get("key_name")?,
            value_enc: self.try_get("value_enc")?,
            crypto_version: self.try_get("crypto_version")?,
            description: self.try_get("description")?,
            tags: parse_tags(&self.try_get::<String, _>("tags")?),
            source: self.try_get("source")?,
            identity: self.try_get("identity").unwrap_or_default(),
            probe_data: parse_probe(&self.try_get::<String, _>("probe_data").unwrap_or_default()),
            created_at: self.try_get("created_at")?,
            updated_at: self.try_get("updated_at")?,
        })
    }
}

impl RowSecret for PgRow {
    fn to_secret(&self) -> anyhow::Result<Secret> {
        Ok(Secret {
            id: self.try_get("id")?,
            project: self.try_get("project")?,
            service: self.try_get("service")?,
            key_name: self.try_get("key_name")?,
            value_enc: self.try_get("value_enc")?,
            crypto_version: self.try_get("crypto_version")?,
            description: self.try_get("description")?,
            tags: parse_tags(&self.try_get::<String, _>("tags")?),
            source: self.try_get("source")?,
            identity: self.try_get("identity").unwrap_or_default(),
            probe_data: parse_probe(&self.try_get::<String, _>("probe_data").unwrap_or_default()),
            created_at: self.try_get("created_at")?,
            updated_at: self.try_get("updated_at")?,
        })
    }
}

fn parse_tags(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

/// 解析 probe_data 列（非机密 JSON 资源/权限清单），空/非法时回退 {}
fn parse_probe(s: &str) -> serde_json::Value {
    if s.is_empty() {
        return serde_json::Value::Null;
    }
    serde_json::from_str(s).unwrap_or(serde_json::Value::Null)
}

fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".into())
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// 生成 uuid
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl Db {
    /// 创建密钥；若 (project,service,key_name) 已存在返回 Ok(None)（由调用方决定 409）
    pub async fn create_secret(
        &self,
        req: &CreateSecretRequest,
        value_enc: &str,
    ) -> anyhow::Result<Option<Secret>> {
        let ts = now();
        let id = new_id();
        let tags = tags_to_json(&req.tags);
        let probe_data = if req.probe_data.is_null() {
            "{}".to_string()
        } else {
            req.probe_data.to_string()
        };
        let n = match self {
            Db::Sqlite(p) => {
                sqlx::query(
                    "INSERT OR IGNORE INTO secrets (id, project, service, key_name, value_enc, crypto_version, description, tags, source, identity, probe_data, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&id).bind(&req.project).bind(&req.service).bind(&req.key_name)
                .bind(value_enc).bind(&req.description).bind(&tags).bind(&req.source)
                .bind(&req.identity).bind(&probe_data).bind(&ts).bind(&ts)
                .execute(p).await?.rows_affected()
            }
            Db::Postgres(p) => {
                sqlx::query(
                    "INSERT INTO secrets (id, project, service, key_name, value_enc, crypto_version, description, tags, source, identity, probe_data, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, $9, $10, $11, $12, $13)
                     ON CONFLICT (project, service, key_name) DO NOTHING",
                )
                .bind(&id).bind(&req.project).bind(&req.service).bind(&req.key_name)
                .bind(value_enc).bind(&req.description).bind(&tags).bind(&req.source)
                .bind(&req.identity).bind(&probe_data).bind(&ts).bind(&ts)
                .execute(p).await?.rows_affected()
            }
        };
        if n == 0 {
            return Ok(None);
        }
        // 读回
        Ok(Some(self.get_secret(&id).await?.unwrap()))
    }

    /// upsert 语义：存在则更新值+元数据，不存在则插入
    pub async fn upsert_secret(
        &self,
        project: &str,
        service: &str,
        key_name: &str,
        value_enc: &str,
        description: &str,
        tags: &[String],
        source: &str,
    ) -> anyhow::Result<Secret> {
        let ts = now();
        let id = new_id();
        let tags_json = tags_to_json(tags);
        let n = match self {
            Db::Sqlite(p) => {
                sqlx::query(
                    "INSERT INTO secrets (id, project, service, key_name, value_enc, crypto_version, description, tags, source, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?)
                     ON CONFLICT (project, service, key_name) DO UPDATE SET
                       value_enc = excluded.value_enc,
                       description = excluded.description,
                       tags = excluded.tags,
                       source = excluded.source,
                       updated_at = excluded.updated_at",
                )
                .bind(&id).bind(project).bind(service).bind(key_name)
                .bind(value_enc).bind(description).bind(&tags_json).bind(source)
                .bind(&ts).bind(&ts)
                .execute(p).await?.rows_affected()
            }
            Db::Postgres(p) => {
                sqlx::query(
                    "INSERT INTO secrets (id, project, service, key_name, value_enc, crypto_version, description, tags, source, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, $9, $10)
                     ON CONFLICT (project, service, key_name) DO UPDATE SET
                       value_enc = EXCLUDED.value_enc,
                       description = EXCLUDED.description,
                       tags = EXCLUDED.tags,
                       source = EXCLUDED.source,
                       updated_at = EXCLUDED.updated_at",
                )
                .bind(&id).bind(project).bind(service).bind(key_name)
                .bind(value_enc).bind(description).bind(&tags_json).bind(source)
                .bind(&ts).bind(&ts)
                .execute(p).await?.rows_affected()
            }
        };
        debug_assert!(n >= 1);
        // 读回（按唯一键）
        self.get_secret_by_key(project, service, key_name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("upsert 后读取失败"))
    }

    /// 写回账号身份（云厂商探测/验证成功后调用）。
    /// service 传空串时搜索整个 project（云密钥常带 service 维度如 oss/cos/gcs）。
    /// 优先写「ID」类记录（如 ALIYUN_ACCESS_KEY_ID / TENCENT_SECRET_ID），否则写第一条。
    pub async fn set_identity(
        &self,
        project: &str,
        service: &str,
        identity: &str,
    ) -> anyhow::Result<bool> {
        if identity.is_empty() {
            return Ok(false);
        }
        let svc = if service.is_empty() {
            None
        } else {
            Some(service)
        };
        let (secrets, _) = self
            .list_secrets(Some(project), svc, None, None, 0, 100)
            .await?;
        let target = secrets
            .iter()
            .find(|s| {
                let k = s.key_name.to_uppercase();
                k.ends_with("_ID") || k.ends_with("_KEY_ID") || k.ends_with("_ACCESS_KEY")
            })
            .or_else(|| secrets.first());
        let Some(target) = target else {
            return Ok(false);
        };
        if target.identity == identity {
            return Ok(false); // 无变化
        }
        self.update_identity(&target.id, identity).await
    }

    /// 更新单条记录的 identity 列
    pub async fn update_identity(&self, id: &str, identity: &str) -> anyhow::Result<bool> {
        let n = match self {
            Db::Sqlite(p) => sqlx::query("UPDATE secrets SET identity = ? WHERE id = ?")
                .bind(identity)
                .bind(id)
                .execute(p)
                .await?
                .rows_affected(),
            Db::Postgres(p) => sqlx::query("UPDATE secrets SET identity = $1 WHERE id = $2")
                .bind(identity)
                .bind(id)
                .execute(p)
                .await?
                .rows_affected(),
        };
        Ok(n > 0)
    }

    /// 写回云厂商探针结果（非机密资源/权限清单 JSON）。写入该 project 下
    /// 「ID」类记录（与 set_identity 同一目标），否则第一条；返回值是否变化。
    pub async fn set_probe_data(
        &self,
        project: &str,
        data: &serde_json::Value,
    ) -> anyhow::Result<bool> {
        if data.is_null() || data.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            return Ok(false);
        }
        let data_str = data.to_string();
        let (secrets, _) = self
            .list_secrets(Some(project), None, None, None, 0, 100)
            .await?;
        let target = secrets
            .iter()
            .find(|s| {
                let k = s.key_name.to_uppercase();
                k.ends_with("_ID") || k.ends_with("_KEY_ID") || k.ends_with("_ACCESS_KEY")
            })
            .or_else(|| secrets.first());
        let Some(target) = target else {
            return Ok(false);
        };
        if target.probe_data.to_string() == data_str {
            return Ok(false);
        }
        let n = match self {
            Db::Sqlite(p) => sqlx::query("UPDATE secrets SET probe_data = ? WHERE id = ?")
                .bind(&data_str)
                .bind(&target.id)
                .execute(p)
                .await?
                .rows_affected(),
            Db::Postgres(p) => sqlx::query("UPDATE secrets SET probe_data = $1 WHERE id = $2")
                .bind(&data_str)
                .bind(&target.id)
                .execute(p)
                .await?
                .rows_affected(),
        };
        Ok(n > 0)
    }

    pub async fn get_secret(&self, id: &str) -> anyhow::Result<Option<Secret>> {
        match self {
            Db::Sqlite(p) => {
                let row = sqlx::query("SELECT * FROM secrets WHERE id = ?")
                    .bind(id)
                    .fetch_optional(p)
                    .await?;
                Ok(row.as_ref().map(|r| r.to_secret()).transpose()?)
            }
            Db::Postgres(p) => {
                let row = sqlx::query("SELECT * FROM secrets WHERE id = $1")
                    .bind(id)
                    .fetch_optional(p)
                    .await?;
                Ok(row.as_ref().map(|r| r.to_secret()).transpose()?)
            }
        }
    }

    pub async fn get_secret_by_key(
        &self,
        project: &str,
        service: &str,
        key_name: &str,
    ) -> anyhow::Result<Option<Secret>> {
        // service 为空时忽略 service 维度（AI 调用通常不关心 service）
        let (sql, bind) =
            if service.is_empty() {
                (
                    "SELECT * FROM secrets WHERE project = ? AND key_name = ? LIMIT 1",
                    vec![project.to_string(), key_name.to_string()],
                )
            } else {
                (
                "SELECT * FROM secrets WHERE project = ? AND service = ? AND key_name = ? LIMIT 1",
                vec![project.to_string(), service.to_string(), key_name.to_string()],
            )
            };
        match self {
            Db::Sqlite(p) => {
                let mut q = sqlx::query(sql);
                for b in &bind {
                    q = q.bind(b);
                }
                let row = q.fetch_optional(p).await?;
                Ok(row.as_ref().map(|r| r.to_secret()).transpose()?)
            }
            Db::Postgres(p) => {
                let pg_sql = rewrite_placeholders(sql);
                let mut q = sqlx::query(&pg_sql);
                for b in &bind {
                    q = q.bind(b);
                }
                let row = q.fetch_optional(p).await?;
                Ok(row.as_ref().map(|r| r.to_secret()).transpose()?)
            }
        }
    }

    /// 列表（仅元数据）。过滤：project/service/搜索词/标签 + 分页
    #[allow(clippy::too_many_arguments)]
    pub async fn list_secrets(
        &self,
        project: Option<&str>,
        service: Option<&str>,
        q: Option<&str>,
        tag: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<(Vec<Secret>, i64)> {
        // 构建 WHERE
        let mut where_clauses: Vec<String> = Vec::new();
        let mut binds: Vec<String> = Vec::new();
        if let Some(p) = project {
            where_clauses.push("project = ?".into());
            binds.push(p.to_string());
        }
        if let Some(s) = service {
            where_clauses.push("service = ?".into());
            binds.push(s.to_string());
        }
        if let Some(q) = q {
            where_clauses.push("(key_name LIKE ? OR description LIKE ? OR project LIKE ?)".into());
            let like = format!("%{q}%");
            binds.push(like.clone());
            binds.push(like.clone());
            binds.push(like.clone());
        }
        if let Some(tag) = tag {
            where_clauses.push("tags LIKE ?".into());
            binds.push(format!("%{tag}%"));
        }
        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };

        let page = page.max(0);
        let page_size = page_size.clamp(1, 500);
        let offset = page * page_size;

        // count
        let count_sql = format!("SELECT COUNT(*) FROM secrets{where_sql}");
        let total = match self {
            Db::Sqlite(p) => {
                let mut q = sqlx::query(&count_sql);
                for b in &binds {
                    q = q.bind(b);
                }
                let row = q.fetch_one(p).await?;
                row.try_get::<i64, _>(0)?
            }
            Db::Postgres(p) => {
                let count_sql_pg = rewrite_placeholders(&count_sql);
                let mut q = sqlx::query(&count_sql_pg);
                for b in &binds {
                    q = q.bind(b);
                }
                let row = q.fetch_one(p).await?;
                row.try_get::<i64, _>(0)?
            }
        };

        // rows：先构造带过滤占位符 + 两个额外 ?（LIMIT/OFFSET）的 SQL，
        // 再按后端重写所有 ? 为 $n（PG）
        let order = " ORDER BY project, service, key_name";
        let select_sql = format!("SELECT * FROM secrets{where_sql}{order} LIMIT ? OFFSET ?");
        let mut secrets = Vec::new();
        match self {
            Db::Sqlite(p) => {
                let mut q = sqlx::query(&select_sql);
                for b in &binds {
                    q = q.bind(b);
                }
                q = q.bind(page_size).bind(offset);
                let rows = q.fetch_all(p).await?;
                for r in &rows {
                    secrets.push(r.to_secret()?);
                }
            }
            Db::Postgres(p) => {
                let select_sql_pg = rewrite_placeholders(&select_sql);
                let mut q = sqlx::query(&select_sql_pg);
                for b in &binds {
                    q = q.bind(b);
                }
                q = q.bind(page_size).bind(offset);
                let rows = q.fetch_all(p).await?;
                for r in &rows {
                    secrets.push(r.to_secret()?);
                }
            }
        }
        Ok((secrets, total))
    }

    /// 更新元数据和/或值；返回是否影响
    pub async fn update_secret(
        &self,
        id: &str,
        req: &UpdateSecretRequest,
        new_value_enc: Option<&str>,
    ) -> anyhow::Result<bool> {
        let ts = now();
        // 构造动态 SET
        let mut sets: Vec<String> = Vec::new();
        let mut binds: Vec<String> = Vec::new();
        if let Some(s) = &req.service {
            add_set(&mut sets, &mut binds, "service", s.clone());
        }
        if let Some(v) = new_value_enc {
            add_set(&mut sets, &mut binds, "value_enc", v.to_string());
            sets.push("crypto_version = 1".into());
        }
        if let Some(d) = &req.description {
            add_set(&mut sets, &mut binds, "description", d.clone());
        }
        if let Some(t) = &req.tags {
            add_set(&mut sets, &mut binds, "tags", tags_to_json(t));
        }
        if sets.is_empty() {
            return Ok(false);
        }
        sets.push("updated_at = ?".into());
        binds.push(ts);
        binds.push(id.to_string());

        let sql = format!("UPDATE secrets SET {} WHERE id = ?", sets.join(", "));
        let n = match self {
            Db::Sqlite(p) => {
                let mut q = sqlx::query(&sql);
                for b in &binds {
                    q = q.bind(b);
                }
                q.execute(p).await?.rows_affected()
            }
            Db::Postgres(p) => {
                // 重写 ? 为 $1..$n
                let sql_pg = rewrite_placeholders(&sql);
                let mut q = sqlx::query(&sql_pg);
                for b in &binds {
                    q = q.bind(b);
                }
                q.execute(p).await?.rows_affected()
            }
        };
        Ok(n > 0)
    }

    pub async fn delete_secret(&self, id: &str) -> anyhow::Result<bool> {
        // 先删关联
        let n = match self {
            Db::Sqlite(p) => {
                let _ = sqlx::query("DELETE FROM collection_items WHERE secret_id = ?")
                    .bind(id)
                    .execute(p)
                    .await?;
                sqlx::query("DELETE FROM secrets WHERE id = ?")
                    .bind(id)
                    .execute(p)
                    .await?
                    .rows_affected()
            }
            Db::Postgres(p) => {
                let _ = sqlx::query("DELETE FROM collection_items WHERE secret_id = $1")
                    .bind(id)
                    .execute(p)
                    .await?;
                sqlx::query("DELETE FROM secrets WHERE id = $1")
                    .bind(id)
                    .execute(p)
                    .await?
                    .rows_affected()
            }
        };
        Ok(n > 0)
    }

    /// 项目树
    pub async fn list_projects(&self) -> anyhow::Result<Vec<ProjectNode>> {
        let sql = "SELECT project, service, COUNT(*) as count FROM secrets GROUP BY project, service ORDER BY project, service";
        let mut map: Vec<(String, Vec<(String, i64)>)> = Vec::new();
        match self {
            Db::Sqlite(p) => {
                let rows = sqlx::query(sql).fetch_all(p).await?;
                for r in rows {
                    let project: String = r.try_get("project")?;
                    let service: String = r.try_get("service")?;
                    let count: i64 = r.try_get("count")?;
                    push_node(&mut map, project, service, count);
                }
            }
            Db::Postgres(p) => {
                let rows = sqlx::query(sql).fetch_all(p).await?;
                for r in rows {
                    let project: String = r.try_get("project")?;
                    let service: String = r.try_get("service")?;
                    let count: i64 = r.try_get("count")?;
                    push_node(&mut map, project, service, count);
                }
            }
        }
        // 读取全部项目图标（meta 键 proj_icon:<project>）
        let icons = self.meta_get_prefix("proj_icon:").await.unwrap_or_default();
        Ok(map
            .into_iter()
            .map(|(project, services)| {
                let count = services.iter().map(|(_, c)| *c).sum();
                let icon = icons.get(&project).cloned().unwrap_or_default();
                ProjectNode {
                    project,
                    services: services.into_iter().map(|(s, _)| s).collect(),
                    count,
                    icon,
                }
            })
            .collect())
    }

    /// 设置项目图标（存 meta；空串表示清除）
    pub async fn set_project_icon(&self, project: &str, icon: &str) -> anyhow::Result<()> {
        let key = format!("proj_icon:{project}");
        if icon.trim().is_empty() {
            self.meta_del(&key).await
        } else {
            self.meta_set(&key, icon.trim()).await
        }
    }

    // ---- meta 键值 ----

    pub async fn meta_get(&self, key: &str) -> anyhow::Result<Option<String>> {
        match self {
            Db::Sqlite(p) => {
                let row = sqlx::query("SELECT v FROM meta WHERE k = ?")
                    .bind(key)
                    .fetch_optional(p)
                    .await?;
                Ok(row.map(|r| r.try_get("v")).transpose()?)
            }
            Db::Postgres(p) => {
                let row = sqlx::query("SELECT v FROM meta WHERE k = $1")
                    .bind(key)
                    .fetch_optional(p)
                    .await?;
                Ok(row.map(|r| r.try_get("v")).transpose()?)
            }
        }
    }

    pub async fn meta_set(&self, key: &str, val: &str) -> anyhow::Result<()> {
        match self {
            Db::Sqlite(p) => {
                sqlx::query("INSERT OR REPLACE INTO meta (k, v) VALUES (?, ?)")
                    .bind(key)
                    .bind(val)
                    .execute(p)
                    .await?;
            }
            Db::Postgres(p) => {
                sqlx::query("INSERT INTO meta (k, v) VALUES ($1, $2) ON CONFLICT (k) DO UPDATE SET v = EXCLUDED.v")
                    .bind(key).bind(val).execute(p).await?;
            }
        }
        Ok(())
    }

    /// 按前缀批量读 meta → {key去掉前缀, value}
    pub async fn meta_get_prefix(
        &self,
        prefix: &str,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        let mut out = std::collections::HashMap::new();
        let pattern = format!("{prefix}%");
        let sql = "SELECT k, v FROM meta WHERE k LIKE ?";
        match self {
            Db::Sqlite(p) => {
                let rows = sqlx::query(sql).bind(&pattern).fetch_all(p).await?;
                for r in rows {
                    let k: String = r.try_get("k")?;
                    let v: String = r.try_get("v")?;
                    if let Some(rest) = k.strip_prefix(prefix) {
                        out.insert(rest.to_string(), v);
                    }
                }
            }
            Db::Postgres(p) => {
                let rows = sqlx::query(sql).bind(&pattern).fetch_all(p).await?;
                for r in rows {
                    let k: String = r.try_get("k")?;
                    let v: String = r.try_get("v")?;
                    if let Some(rest) = k.strip_prefix(prefix) {
                        out.insert(rest.to_string(), v);
                    }
                }
            }
        }
        Ok(out)
    }

    pub async fn meta_del(&self, key: &str) -> anyhow::Result<()> {
        match self {
            Db::Sqlite(p) => {
                sqlx::query("DELETE FROM meta WHERE k = ?")
                    .bind(key)
                    .execute(p)
                    .await?;
            }
            Db::Postgres(p) => {
                sqlx::query("DELETE FROM meta WHERE k = $1")
                    .bind(key)
                    .execute(p)
                    .await?;
            }
        }
        Ok(())
    }

    // ---- API keys ----

    pub async fn create_api_key(
        &self,
        name: &str,
        key_hash: &str,
        scopes: &[String],
    ) -> anyhow::Result<ApiKey> {
        let id = new_id();
        let ts = now();
        let scopes_json = serde_json::to_string(scopes).unwrap_or_else(|_| "[]".into());
        match self {
            Db::Sqlite(p) => {
                sqlx::query("INSERT INTO api_keys (id, name, key_hash, scopes, created_at) VALUES (?, ?, ?, ?, ?)")
                    .bind(&id).bind(name).bind(key_hash).bind(&scopes_json).bind(&ts).execute(p).await?;
            }
            Db::Postgres(p) => {
                sqlx::query("INSERT INTO api_keys (id, name, key_hash, scopes, created_at) VALUES ($1, $2, $3, $4, $5)")
                    .bind(&id).bind(name).bind(key_hash).bind(&scopes_json).bind(&ts).execute(p).await?;
            }
        }
        self.get_api_key(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("创建失败"))
    }

    pub async fn get_api_key(&self, id: &str) -> anyhow::Result<Option<ApiKey>> {
        match self {
            Db::Sqlite(p) => {
                let row = sqlx::query("SELECT * FROM api_keys WHERE id = ?")
                    .bind(id)
                    .fetch_optional(p)
                    .await?;
                Ok(row.map(|r| api_key_from_sqlite(&r)).transpose()?)
            }
            Db::Postgres(p) => {
                let row = sqlx::query("SELECT * FROM api_keys WHERE id = $1")
                    .bind(id)
                    .fetch_optional(p)
                    .await?;
                Ok(row.map(|r| api_key_from_pg(&r)).transpose()?)
            }
        }
    }

    pub async fn get_api_key_by_hash(&self, key_hash: &str) -> anyhow::Result<Option<ApiKey>> {
        match self {
            Db::Sqlite(p) => {
                let row = sqlx::query("SELECT * FROM api_keys WHERE key_hash = ?")
                    .bind(key_hash)
                    .fetch_optional(p)
                    .await?;
                Ok(row.map(|r| api_key_from_sqlite(&r)).transpose()?)
            }
            Db::Postgres(p) => {
                let row = sqlx::query("SELECT * FROM api_keys WHERE key_hash = $1")
                    .bind(key_hash)
                    .fetch_optional(p)
                    .await?;
                Ok(row.map(|r| api_key_from_pg(&r)).transpose()?)
            }
        }
    }

    pub async fn list_api_keys(&self) -> anyhow::Result<Vec<ApiKey>> {
        let mut out = Vec::new();
        match self {
            Db::Sqlite(p) => {
                let rows = sqlx::query("SELECT * FROM api_keys ORDER BY created_at DESC")
                    .fetch_all(p)
                    .await?;
                for r in rows {
                    out.push(ApiKey {
                        id: r.try_get("id")?,
                        name: r.try_get("name")?,
                        key_hash: r.try_get("key_hash")?,
                        scopes: parse_tags(&r.try_get::<String, _>("scopes")?),
                        created_at: r.try_get("created_at")?,
                        last_used_at: r.try_get("last_used_at")?,
                    });
                }
            }
            Db::Postgres(p) => {
                let rows = sqlx::query("SELECT * FROM api_keys ORDER BY created_at DESC")
                    .fetch_all(p)
                    .await?;
                for r in rows {
                    out.push(ApiKey {
                        id: r.try_get("id")?,
                        name: r.try_get("name")?,
                        key_hash: r.try_get("key_hash")?,
                        scopes: parse_tags(&r.try_get::<String, _>("scopes")?),
                        created_at: r.try_get("created_at")?,
                        last_used_at: r.try_get("last_used_at")?,
                    });
                }
            }
        }
        Ok(out)
    }

    pub async fn delete_api_key(&self, id: &str) -> anyhow::Result<bool> {
        let n = match self {
            Db::Sqlite(p) => sqlx::query("DELETE FROM api_keys WHERE id = ?")
                .bind(id)
                .execute(p)
                .await?
                .rows_affected(),
            Db::Postgres(p) => sqlx::query("DELETE FROM api_keys WHERE id = $1")
                .bind(id)
                .execute(p)
                .await?
                .rows_affected(),
        };
        Ok(n > 0)
    }

    pub async fn touch_api_key(&self, id: &str) -> anyhow::Result<()> {
        let ts = now();
        match self {
            Db::Sqlite(p) => {
                sqlx::query("UPDATE api_keys SET last_used_at = ? WHERE id = ?")
                    .bind(&ts)
                    .bind(id)
                    .execute(p)
                    .await?;
            }
            Db::Postgres(p) => {
                sqlx::query("UPDATE api_keys SET last_used_at = $1 WHERE id = $2")
                    .bind(&ts)
                    .bind(id)
                    .execute(p)
                    .await?;
            }
        }
        Ok(())
    }

    // ---- 审计 ----

    pub async fn audit(
        &self,
        actor: &str,
        action: &str,
        secret_id: Option<&str>,
        ip: &str,
    ) -> anyhow::Result<()> {
        let ts = now();
        match self {
            Db::Sqlite(p) => {
                sqlx::query("INSERT INTO audit_log (actor, action, secret_id, at, ip) VALUES (?, ?, ?, ?, ?)")
                    .bind(actor).bind(action).bind(secret_id).bind(&ts).bind(ip).execute(p).await?;
            }
            Db::Postgres(p) => {
                sqlx::query("INSERT INTO audit_log (actor, action, secret_id, at, ip) VALUES ($1, $2, $3, $4, $5)")
                    .bind(actor).bind(action).bind(secret_id).bind(&ts).bind(ip).execute(p).await?;
            }
        }
        Ok(())
    }

    // ---- collections 分组 ----

    pub async fn create_collection(
        &self,
        name: &str,
        description: &str,
    ) -> anyhow::Result<Collection> {
        let id = new_id();
        let ts = now();
        match self {
            Db::Sqlite(p) => {
                sqlx::query("INSERT INTO collections (id, name, description, created_at) VALUES (?, ?, ?, ?)")
                    .bind(&id).bind(name).bind(description).bind(&ts).execute(p).await?;
            }
            Db::Postgres(p) => {
                sqlx::query("INSERT INTO collections (id, name, description, created_at) VALUES ($1, $2, $3, $4)")
                    .bind(&id).bind(name).bind(description).bind(&ts).execute(p).await?;
            }
        }
        Ok(Collection {
            id,
            name: name.to_string(),
            description: description.to_string(),
            created_at: ts,
        })
    }

    pub async fn list_collections(&self) -> anyhow::Result<Vec<Collection>> {
        let mut out = Vec::new();
        match self {
            Db::Sqlite(p) => {
                let rows = sqlx::query("SELECT * FROM collections ORDER BY name")
                    .fetch_all(p)
                    .await?;
                for r in rows {
                    out.push(Collection {
                        id: r.try_get("id")?,
                        name: r.try_get("name")?,
                        description: r.try_get("description")?,
                        created_at: r.try_get("created_at")?,
                    });
                }
            }
            Db::Postgres(p) => {
                let rows = sqlx::query("SELECT * FROM collections ORDER BY name")
                    .fetch_all(p)
                    .await?;
                for r in rows {
                    out.push(Collection {
                        id: r.try_get("id")?,
                        name: r.try_get("name")?,
                        description: r.try_get("description")?,
                        created_at: r.try_get("created_at")?,
                    });
                }
            }
        }
        Ok(out)
    }

    pub async fn delete_collection(&self, id: &str) -> anyhow::Result<bool> {
        let n = match self {
            Db::Sqlite(p) => {
                let _ = sqlx::query("DELETE FROM collection_items WHERE collection_id = ?")
                    .bind(id)
                    .execute(p)
                    .await?;
                sqlx::query("DELETE FROM collections WHERE id = ?")
                    .bind(id)
                    .execute(p)
                    .await?
                    .rows_affected()
            }
            Db::Postgres(p) => {
                let _ = sqlx::query("DELETE FROM collection_items WHERE collection_id = $1")
                    .bind(id)
                    .execute(p)
                    .await?;
                sqlx::query("DELETE FROM collections WHERE id = $1")
                    .bind(id)
                    .execute(p)
                    .await?
                    .rows_affected()
            }
        };
        Ok(n > 0)
    }

    pub async fn add_item(&self, collection_id: &str, secret_id: &str) -> anyhow::Result<bool> {
        let n = match self {
            Db::Sqlite(p) => {
                sqlx::query("INSERT OR IGNORE INTO collection_items (collection_id, secret_id) VALUES (?, ?)")
                    .bind(collection_id).bind(secret_id).execute(p).await?.rows_affected()
            }
            Db::Postgres(p) => {
                sqlx::query("INSERT INTO collection_items (collection_id, secret_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                    .bind(collection_id).bind(secret_id).execute(p).await?.rows_affected()
            }
        };
        Ok(n > 0)
    }

    pub async fn remove_item(&self, collection_id: &str, secret_id: &str) -> anyhow::Result<bool> {
        let n = match self {
            Db::Sqlite(p) => sqlx::query(
                "DELETE FROM collection_items WHERE collection_id = ? AND secret_id = ?",
            )
            .bind(collection_id)
            .bind(secret_id)
            .execute(p)
            .await?
            .rows_affected(),
            Db::Postgres(p) => sqlx::query(
                "DELETE FROM collection_items WHERE collection_id = $1 AND secret_id = $2",
            )
            .bind(collection_id)
            .bind(secret_id)
            .execute(p)
            .await?
            .rows_affected(),
        };
        Ok(n > 0)
    }

    pub async fn list_collection_items(
        &self,
        collection_id: &str,
    ) -> anyhow::Result<Vec<SecretMeta>> {
        let sql = "SELECT s.* FROM collection_items ci JOIN secrets s ON s.id = ci.secret_id WHERE ci.collection_id = ? ORDER BY s.project, s.key_name";
        let mut out = Vec::new();
        match self {
            Db::Sqlite(p) => {
                let rows = sqlx::query(sql).bind(collection_id).fetch_all(p).await?;
                for r in rows {
                    out.push(r.to_secret()?.to_meta());
                }
            }
            Db::Postgres(p) => {
                let sql_pg = rewrite_placeholders(sql);
                let rows = sqlx::query(&sql_pg)
                    .bind(collection_id)
                    .fetch_all(p)
                    .await?;
                for r in rows {
                    out.push(r.to_secret()?.to_meta());
                }
            }
        }
        Ok(out)
    }
}

fn add_set(sets: &mut Vec<String>, binds: &mut Vec<String>, col: &str, val: String) {
    sets.push(format!("{col} = ?"));
    binds.push(val);
}

fn api_key_from_sqlite(r: &SqliteRow) -> anyhow::Result<ApiKey> {
    Ok(ApiKey {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        key_hash: r.try_get("key_hash")?,
        scopes: parse_tags(&r.try_get::<String, _>("scopes")?),
        created_at: r.try_get("created_at")?,
        last_used_at: r.try_get("last_used_at")?,
    })
}

fn api_key_from_pg(r: &PgRow) -> anyhow::Result<ApiKey> {
    Ok(ApiKey {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        key_hash: r.try_get("key_hash")?,
        scopes: parse_tags(&r.try_get::<String, _>("scopes")?),
        created_at: r.try_get("created_at")?,
        last_used_at: r.try_get("last_used_at")?,
    })
}

fn push_node(
    map: &mut Vec<(String, Vec<(String, i64)>)>,
    project: String,
    service: String,
    count: i64,
) {
    if let Some(entry) = map.iter_mut().find(|(p, _)| *p == project) {
        entry.1.push((service, count));
    } else {
        map.push((project, vec![(service, count)]));
    }
}

/// 把 `?` 占位符重写为 $1,$2,...（仅用于动态 SQL 数量固定场景）
fn rewrite_placeholders(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut n = 0;
    for ch in sql.chars() {
        if ch == '?' {
            n += 1;
            out.push_str(&format!("${n}"));
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CreateSecretRequest;

    async fn db() -> Db {
        crate::db::test_db().await
    }

    #[tokio::test]
    async fn create_and_get() {
        let db = db().await;
        let req = CreateSecretRequest {
            project: "test".into(),
            service: "svc".into(),
            key_name: "API_KEY".into(),
            value: "sk-test".into(),
            description: "desc".into(),
            tags: vec!["ai".into()],
            source: "test".into(),
            identity: String::new(),
            probe_data: serde_json::Value::Null,
        };
        let s = db.create_secret(&req, "ENC").await.unwrap().unwrap();
        assert_eq!(s.project, "test");
        assert_eq!(s.key_name, "API_KEY");
        let got = db.get_secret(&s.id).await.unwrap().unwrap();
        assert_eq!(got.value_enc, "ENC");
    }

    #[tokio::test]
    async fn duplicate_create_conflicts() {
        let db = db().await;
        let req = CreateSecretRequest {
            project: "test".into(),
            service: "".into(),
            key_name: "K".into(),
            value: "v".into(),
            description: String::new(),
            tags: vec![],
            source: String::new(),
            identity: String::new(),
            probe_data: serde_json::Value::Null,
        };
        assert!(db.create_secret(&req, "ENC1").await.unwrap().is_some());
        assert!(db.create_secret(&req, "ENC2").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn upsert_updates() {
        let db = db().await;
        let s1 = db
            .upsert_secret("p", "s", "K", "ENC1", "", &[], "src")
            .await
            .unwrap();
        let s2 = db
            .upsert_secret("p", "s", "K", "ENC2", "newdesc", &["tag".into()], "src2")
            .await
            .unwrap();
        assert_eq!(s1.id, s2.id);
        assert_eq!(s2.value_enc, "ENC2");
        assert_eq!(s2.description, "newdesc");
    }

    #[tokio::test]
    async fn list_filters() {
        let db = db().await;
        for i in 0..3 {
            db.upsert_secret(
                "proj",
                "svc",
                &format!("KEY_{i}"),
                "ENC",
                "desc",
                &["ai".into()],
                "",
            )
            .await
            .unwrap();
        }
        db.upsert_secret("other", "s2", "TOKEN", "ENC", "x", &["cloud".into()], "")
            .await
            .unwrap();
        let (list, total) = db
            .list_secrets(Some("proj"), None, None, None, 0, 10)
            .await
            .unwrap();
        assert_eq!(total, 3);
        assert_eq!(list.len(), 3);
        let (_, t2) = db
            .list_secrets(None, None, Some("TOKEN"), None, 0, 10)
            .await
            .unwrap();
        assert_eq!(t2, 1);
        let (_, t3) = db
            .list_secrets(None, None, None, Some("cloud"), 0, 10)
            .await
            .unwrap();
        assert_eq!(t3, 1);
        // 分页
        let (page, t4) = db.list_secrets(None, None, None, None, 0, 2).await.unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(t4, 4);
    }

    #[tokio::test]
    async fn update_and_delete() {
        let db = db().await;
        let s = db
            .upsert_secret("p", "s", "K", "ENC1", "", &[], "")
            .await
            .unwrap();
        let req = UpdateSecretRequest {
            service: Some("s2".into()),
            value: None,
            description: Some("updated".into()),
            tags: None,
        };
        assert!(db.update_secret(&s.id, &req, None).await.unwrap());
        let got = db.get_secret(&s.id).await.unwrap().unwrap();
        assert_eq!(got.service, "s2");
        assert_eq!(got.description, "updated");
        assert!(db.delete_secret(&s.id).await.unwrap());
        assert!(db.get_secret(&s.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn projects_tree() {
        let db = db().await;
        db.upsert_secret("a", "s1", "K1", "ENC", "", &[], "")
            .await
            .unwrap();
        db.upsert_secret("a", "s1", "K2", "ENC", "", &[], "")
            .await
            .unwrap();
        db.upsert_secret("a", "s2", "K3", "ENC", "", &[], "")
            .await
            .unwrap();
        let tree = db.list_projects().await.unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].count, 3);
        assert_eq!(tree[0].services.len(), 2);
    }

    #[tokio::test]
    async fn api_key_crud() {
        let db = db().await;
        let k = db
            .create_api_key("test", "hash123", &["read".into(), "write".into()])
            .await
            .unwrap();
        assert_eq!(k.scopes.len(), 2);
        let by_hash = db.get_api_key_by_hash("hash123").await.unwrap().unwrap();
        assert_eq!(by_hash.id, k.id);
        assert!(db.delete_api_key(&k.id).await.unwrap());
        assert!(db.get_api_key(&k.id).await.unwrap().is_none());
    }
}
