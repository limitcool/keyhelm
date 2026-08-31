//! Keyhelm CLI 入口：serve | import | bootstrap | gen-key | gen-token

use clap::{Parser, Subcommand};
use keyhelm::api::auth::{generate_api_key_token, hash_token, load_jwt_secret, sign_jwt, Claims};
use keyhelm::api::AppState;
use keyhelm::config::Config;
use keyhelm::crypto::{self, MasterKey};
use keyhelm::db::Db;
use keyhelm::import;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "keyhelm",
    version,
    about = "密钥配置中心 — AI 可读写的统一密钥管理"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动服务（REST API + Web UI）
    Serve,
    /// 从 docker-stacks / .secrets 聚合导入密钥
    Import {
        /// 扫描所有 compose 和 .env
        #[arg(long)]
        all: bool,
        /// 仅打印将要导入的内容，不写库
        #[arg(long)]
        dry_run: bool,
        /// 指定 docker-stacks 根目录
        #[arg(long)]
        stacks_dir: Option<PathBuf>,
        /// 指定 secrets 目录
        #[arg(long)]
        secrets_dir: Option<PathBuf>,
    },
    /// 初始化 admin 密码与 API token（首次运行）
    Bootstrap,
    /// 设置 admin 密码（覆盖已有）
    SetPassword {
        /// 新密码
        password: String,
    },
    /// 生成主密钥（master key）
    GenKey {
        /// 输出路径（默认 data/master.key）
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// 生成 JWT 签名密钥
    GenJwt {
        /// 输出路径（默认 data/jwt.secret）
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// 生成一个 API token（需要先有运行中的服务）
    GenToken {
        /// 名字
        name: String,
        /// 权限，逗号分隔：read,write,admin
        #[arg(long, default_value = "read")]
        scopes: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "keyhelm=info,tower_http=info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Serve => cmd_serve().await?,
        Commands::Import {
            all,
            dry_run,
            stacks_dir,
            secrets_dir,
        } => cmd_import(all, dry_run, stacks_dir, secrets_dir).await?,
        Commands::Bootstrap => cmd_bootstrap().await?,
        Commands::SetPassword { password } => cmd_set_password(&password).await?,
        Commands::GenKey { out } => cmd_gen_key(out)?,
        Commands::GenJwt { out } => cmd_gen_jwt(out)?,
        Commands::GenToken { name, scopes } => cmd_gen_token(&name, &scopes).await?,
    }
    Ok(())
}

/// 加载配置 + 数据库 + 主密钥 + JWT 密钥，组装 AppState
async fn load_state(cfg: Config) -> anyhow::Result<AppState> {
    let db = Db::connect(&cfg.db).await?;
    let master_key: MasterKey = crypto::load_master_key(&cfg.crypto)?;
    let jwt_secret = load_jwt_secret(&cfg.auth.jwt_secret_env, &cfg.auth.jwt_secret_file)?;
    // bootstrap 检查：若未初始化 admin，自动生成
    ensure_bootstrap(&db, &cfg).await?;
    Ok(AppState {
        db,
        master_key,
        jwt_secret: Arc::new(jwt_secret),
        cfg: Arc::new(cfg),
    })
}

async fn cmd_serve() -> anyhow::Result<()> {
    let cfg = Config::load()?;
    let state = load_state(cfg.clone()).await?;
    let bind = state.cfg.server.bind_addr.clone();
    let app = keyhelm::api::build_router(state);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("Keyhelm 服务启动: http://{bind}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn cmd_import(
    all: bool,
    dry_run: bool,
    stacks_dir: Option<PathBuf>,
    secrets_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let mut cfg = Config::load()?;
    if let Some(d) = stacks_dir {
        cfg.import.docker_stacks_dir = d;
    }
    if let Some(d) = secrets_dir {
        cfg.import.secrets_dir = d;
    }
    let db = Db::connect(&cfg.db).await?;
    let master_key: MasterKey = crypto::load_master_key(&cfg.crypto)?;
    import::run(&db, &master_key, &cfg, all, dry_run).await
}

async fn cmd_bootstrap() -> anyhow::Result<()> {
    let cfg = Config::load()?;
    let db = Db::connect(&cfg.db).await?;
    ensure_bootstrap(&db, &cfg).await?;
    Ok(())
}

/// 覆盖 admin 密码（写入 argon2 hash 到 meta 表）
async fn cmd_set_password(password: &str) -> anyhow::Result<()> {
    if password.trim().is_empty() {
        anyhow::bail!("密码不能为空");
    }
    let cfg = Config::load()?;
    let db = Db::connect(&cfg.db).await?;
    let hash = hash_password(password)?;
    db.meta_set("admin_password_hash", &hash).await?;
    db.meta_set("admin_initialized", "1").await?;
    println!("admin 密码已更新（{password}）");
    Ok(())
}

/// 首次运行：生成随机 admin 密码 + 初始 admin API token，打印一次
async fn ensure_bootstrap(db: &Db, cfg: &Config) -> anyhow::Result<()> {
    // 已在 meta 标记初始化则跳过
    if db.meta_get("admin_initialized").await?.as_deref() == Some("1") {
        return Ok(());
    }
    // admin 密码
    if cfg.auth.admin_password_hash.is_empty() {
        if db.meta_get("admin_password_hash").await?.is_none() {
            let password = generate_api_key_token(); // 复用随机生成
            let hash = hash_password(&password)?;
            db.meta_set("admin_password_hash", &hash).await?;
            // 打印一次（日志里只出现这一次）
            tracing::warn!("================================================================");
            tracing::warn!("🔑 初始 admin 密码（仅此一次，请立即保存）:");
            tracing::warn!("   username: {}", cfg.auth.admin_username);
            tracing::warn!("   password: {password}");
            tracing::warn!("================================================================");
        }
    }
    // 初始 admin API token
    if db.list_api_keys().await?.is_empty() {
        let token = generate_api_key_token();
        let hash = hash_token(&token);
        db.create_api_key(
            "bootstrap-admin",
            &hash,
            &["read".into(), "write".into(), "admin".into()],
        )
        .await?;
        tracing::warn!("================================================================");
        tracing::warn!("🔑 初始 admin API token（仅此一次）:");
        tracing::warn!("   {token}");
        tracing::warn!("   （用它 POST /api/v1/token 换取 JWT，或直接 Bearer 使用）");
        tracing::warn!("================================================================");
    }
    db.meta_set("admin_initialized", "1").await?;
    Ok(())
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    use argon2::Argon2;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("argon2: {e}"))
}

fn cmd_gen_key(out: Option<PathBuf>) -> anyhow::Result<()> {
    let cfg = Config::load()?;
    let path = out.unwrap_or_else(|| cfg.crypto.master_key_file.clone());
    let hex = crypto::generate_master_key_file(&path)?;
    println!("主密钥已写入: {}", path.display());
    println!("也可以直接用于 env 变量:");
    println!("KEYHELM_MASTER_KEY={hex}");
    Ok(())
}

fn cmd_gen_jwt(out: Option<PathBuf>) -> anyhow::Result<()> {
    let cfg = Config::load()?;
    let path = out.unwrap_or_else(|| cfg.auth.jwt_secret_file.clone());
    let secret: Vec<u8> = (0..48).map(|_| rand::random::<u8>()).collect();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &secret)?;
    println!("JWT 签名密钥已写入: {}", path.display());
    Ok(())
}

/// 生成 API token：直接用明文（不写库，用户自己拿到 API 建）
async fn cmd_gen_token(name: &str, scopes: &str) -> anyhow::Result<()> {
    let cfg = Config::load()?;
    let db = Db::connect(&cfg.db).await?;
    let master_key: MasterKey = crypto::load_master_key(&cfg.crypto)?;
    let jwt_secret = load_jwt_secret(&cfg.auth.jwt_secret_env, &cfg.auth.jwt_secret_file)?;
    let scope_list: Vec<String> = scopes
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let token = generate_api_key_token();
    let hash = hash_token(&token);
    let key = db.create_api_key(name, &hash, &scope_list).await?;
    println!("API token 已创建: {}", key.name);
    println!("明文 token（仅此一次）: {token}");
    println!(
        "使用方式: curl -H 'Authorization: Bearer {}' ... 或先 POST /api/v1/token 换取 JWT",
        sign_jwt(
            &jwt_secret,
            &Claims {
                sub: key.id,
                exp: chrono::Utc::now().timestamp() as usize
                    + cfg.auth.session_ttl_secs.max(60) as usize,
                scopes: scope_list,
                key_name: name.to_string(),
            }
        )?
    );
    let _ = master_key;
    Ok(())
}
