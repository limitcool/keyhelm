//! 配置加载：config.yaml + KEYHELM_* 环境变量覆盖

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub db: DbConfig,
    pub crypto: CryptoConfig,
    pub auth: AuthConfig,
    pub import: ImportConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DbConfig {
    pub kind: DbKind,
    pub sqlite_path: PathBuf,
    pub postgres_url: String,
    pub pool_max_conns: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DbKind {
    #[default]
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CryptoConfig {
    /// 从哪个环境变量读取主密钥值
    pub master_key_env: String,
    /// 主密钥文件（env 未提供时读取）
    pub master_key_file: PathBuf,
    /// 直接内联的主密钥值（KEYHELM_MASTER_KEY 覆盖时填充）
    pub master_key_value: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AuthConfig {
    pub admin_username: String,
    pub admin_password_hash: String,
    pub jwt_secret_env: String,
    pub jwt_secret_file: PathBuf,
    pub session_ttl_secs: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ImportConfig {
    pub docker_stacks_dir: PathBuf,
    pub secrets_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                bind_addr: "0.0.0.0:8080".into(),
                cors_origins: Vec::new(),
            },
            db: DbConfig {
                kind: DbKind::Sqlite,
                sqlite_path: PathBuf::from("./data/keyhelm.db"),
                postgres_url: "postgres://keyhelm:CHANGE_ME@localhost:5432/keyhelm".into(),
                pool_max_conns: 5,
            },
            crypto: CryptoConfig {
                master_key_env: "KEYHELM_MASTER_KEY".into(),
                master_key_file: PathBuf::from("./data/master.key"),
                master_key_value: String::new(),
            },
            auth: AuthConfig {
                admin_username: "admin".into(),
                admin_password_hash: String::new(),
                jwt_secret_env: "KEYHELM_JWT_SECRET".into(),
                jwt_secret_file: PathBuf::from("./data/jwt.secret"),
                session_ttl_secs: 3600,
            },
            import: ImportConfig {
                docker_stacks_dir: PathBuf::from("/opt/docker-stacks"),
                secrets_dir: PathBuf::from("/root/.secrets"),
            },
        }
    }
}

impl Config {
    /// 从文件加载配置，路径默认 ./config.yaml，可用 KEYHELM_CONFIG 覆盖
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("KEYHELM_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./config.yaml"));

        let mut cfg = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            serde_yaml::from_str::<Config>(&raw)?
        } else {
            tracing::warn!("config.yaml 不存在，使用默认配置");
            Config::default()
        };

        cfg.apply_env_overrides();
        Ok(cfg)
    }

    /// 环境变量覆盖（仅覆盖已设置的项）
    fn apply_env_overrides(&mut self) {
        get_env("KEYHELM_BIND_ADDR").map(|v| self.server.bind_addr = v);
        get_env("KEYHELM_DB_KIND").map(|v| {
            self.db.kind = match v.as_str() {
                "postgres" | "pg" => DbKind::Postgres,
                _ => DbKind::Sqlite,
            }
        });
        get_env("KEYHELM_DB_PATH").map(|v| self.db.sqlite_path = PathBuf::from(v));
        get_env("KEYHELM_DB_URL").map(|v| self.db.postgres_url = v);
        if let Ok(url) = std::env::var("DATABASE_URL") {
            if self.db.kind == DbKind::Postgres {
                self.db.postgres_url = url;
            }
        }
        get_env("KEYHELM_MASTER_KEY").map(|v| self.crypto.master_key_value = v);
        get_env("KEYHELM_MASTER_KEY_FILE").map(|v| self.crypto.master_key_file = PathBuf::from(v));
        get_env("KEYHELM_ADMIN_USERNAME").map(|v| self.auth.admin_username = v);
        get_env("KEYHELM_ADMIN_PASSWORD_HASH").map(|v| self.auth.admin_password_hash = v);
        get_env("KEYHELM_JWT_SECRET").map(|v| self.auth.jwt_secret_env = v);
        get_env("KEYHELM_JWT_SECRET_FILE").map(|v| self.auth.jwt_secret_file = PathBuf::from(v));
        get_env("KEYHELM_SESSION_TTL")
            .and_then(|v| v.parse().ok())
            .map(|v| self.auth.session_ttl_secs = v);
    }
}

fn get_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}
