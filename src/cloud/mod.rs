//! 云厂商集成：验证 key 有效性 + 探测可访问资源
//!
//! 每个 provider 实现 `verify`（确认 key 有效并返回账号/权限概览）和
//! `probe`（尝试列出 key 可访问的资源）。密钥来自 Keyhelm 中对应 project
//! 下的明文（自动解密）。

pub mod providers;

use std::collections::HashMap;

use serde_json::Value;

use crate::crypto::{self, MasterKey};
use crate::db::Db;

/// 一个云厂商的 key 集合（按 key_name 取明文）
pub struct CloudKeys {
    pub provider: String,
    pub keys: HashMap<String, String>,
}

/// 从 project 下读取密钥并解密，组装为 CloudKeys
pub async fn load_cloud_keys(
    db: &Db,
    master_key: &MasterKey,
    project: &str,
) -> anyhow::Result<CloudKeys> {
    let (secrets, _) = db
        .list_secrets(Some(project), None, None, None, 0, 500)
        .await?;
    let mut keys = HashMap::new();
    for s in &secrets {
        match crypto::decrypt(master_key, &s.value_enc) {
            Ok(v) => {
                keys.insert(s.key_name.clone(), v);
            }
            Err(e) => tracing::warn!("cloud load {}: {e}", s.key_name),
        }
    }
    if keys.is_empty() {
        anyhow::bail!("project {project} 下没有可用密钥");
    }
    Ok(CloudKeys {
        provider: project.to_string(),
        keys,
    })
}

/// 统一验证入口：按 provider 分派
pub async fn verify(provider: &str, keys: &CloudKeys) -> Result<Value, String> {
    match provider {
        "aliyun" => providers::aliyun_verify(keys).await,
        "tencent" => providers::tencent_verify(keys).await,
        "cloudflare" => providers::cloudflare_verify(keys).await,
        "google-cloud" => providers::google_verify(keys).await,
        other => Err(format!("不支持的云厂商: {other}")),
    }
}

/// 统一探测入口
pub async fn probe(provider: &str, keys: &CloudKeys) -> Result<Value, String> {
    match provider {
        "aliyun" => providers::aliyun_probe(keys).await,
        "tencent" => providers::tencent_probe(keys).await,
        "cloudflare" => providers::cloudflare_probe(keys).await,
        "google-cloud" => providers::google_probe(keys).await,
        other => Err(format!("不支持的云厂商: {other}")),
    }
}
