//! 导入工具：从 docker-stacks / .secrets 聚合散落密钥

pub mod docker_compose;
pub mod dotenv;
pub mod yamlcfg;

use std::path::PathBuf;

use crate::config::Config;
use crate::crypto::MasterKey;
use crate::db::Db;

/// 一条待导入的键值
pub struct Candidate {
    pub project: String,
    pub service: String,
    pub key_name: String,
    pub value: String,
    pub description: String,
    pub source: String,
}

/// 运行导入
pub async fn run(
    db: &Db,
    master_key: &MasterKey,
    cfg: &Config,
    all: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    let mut candidates: Vec<Candidate> = Vec::new();

    if all || cfg.import.docker_stacks_dir.exists() {
        // 扫描 docker-stacks 下所有 compose 文件
        let mut found = 0usize;
        for entry in walk_dir(&cfg.import.docker_stacks_dir) {
            let name = entry
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name == "docker-compose.yaml"
                || name == "docker-compose.yml"
                || name == "compose.yaml"
                || name == "compose.yml"
            {
                if let Some(dir) = entry.parent() {
                    let project = dir
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "unknown".into());
                    match docker_compose::parse(&entry, &project) {
                        Ok(mut items) => {
                            found += items.len();
                            candidates.append(&mut items);
                        }
                        Err(e) => tracing::warn!("解析 {} 失败: {e}", entry.display()),
                    }
                    // 同目录下可能还有 config.yaml（如 grok2api/config.yaml）
                    let cfg_yaml = dir.join("config.yaml");
                    if cfg_yaml.is_file() {
                        match yamlcfg::parse(&cfg_yaml, &project) {
                            Ok(mut items) => {
                                found += items.len();
                                candidates.append(&mut items);
                            }
                            Err(e) => tracing::warn!("解析 {} 失败: {e}", cfg_yaml.display()),
                        }
                    }
                }
            }
        }
        tracing::info!("docker-stacks 扫描完成，发现 {found} 条");
    }

    // 扫描 secrets 目录 .env
    if cfg.import.secrets_dir.exists() {
        let mut found = 0usize;
        for entry in walk_dir(&cfg.import.secrets_dir) {
            if entry.is_file() && entry.extension().map(|e| e == "env").unwrap_or(false) {
                let project = entry
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "secrets".into());
                match dotenv::parse(&entry, &project) {
                    Ok(mut items) => {
                        found += items.len();
                        candidates.append(&mut items);
                    }
                    Err(e) => tracing::warn!("解析 {} 失败: {e}", entry.display()),
                }
            }
        }
        tracing::info!(".secrets 扫描完成，发现 {found} 条");
    }

    if candidates.is_empty() {
        tracing::warn!("未发现可导入的密钥");
        return Ok(());
    }

    if dry_run {
        println!("=== Dry run: 将导入 {} 条 ===", candidates.len());
        for c in &candidates {
            println!(
                "  [{}] {}.{}.{} = {}",
                c.source,
                c.project,
                c.service,
                c.key_name,
                mask(&c.value)
            );
        }
        return Ok(());
    }

    // 写入（upsert）
    let mut written = 0usize;
    for c in &candidates {
        let enc = match crate::crypto::encrypt(master_key, &c.value) {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!("加密 {} 失败: {err}", c.key_name);
                continue;
            }
        };
        match db
            .upsert_secret(
                &c.project,
                &c.service,
                &c.key_name,
                &enc,
                &c.description,
                &[],
                &c.source,
            )
            .await
        {
            Ok(_) => written += 1,
            Err(e) => tracing::warn!("写入 {}.{} 失败: {e}", c.project, c.key_name),
        }
    }
    tracing::info!("导入完成：写入 {written} 条（总候选 {}）", candidates.len());
    Ok(())
}

fn walk_dir(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk_dir(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

/// 脱敏显示：只显示前后各 4 字符
fn mask(v: &str) -> String {
    if v.len() <= 12 {
        "********".to_string()
    } else {
        format!("{}…{}", &v[..4], &v[v.len() - 4..])
    }
}
