//! 数据模型（所有密钥以密文存储）

use serde::{Deserialize, Serialize};

/// 一条密钥记录（value 字段永远不含明文，见 value_enc）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    pub id: String,
    pub project: String,
    pub service: String,
    pub key_name: String,
    pub value_enc: String,
    pub crypto_version: i32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub source: String,
    /// 该密钥对应的账号身份（云厂商探测后写回，如 RAM 用户名 / 腾讯 AccountId / GCP client_email）。
    /// 仅存身份标识（非机密），用于卡片直接展示。
    #[serde(default)]
    pub identity: String,
    /// 云厂商探测到的可访问资源/权限（非机密 JSON：buckets/policies/zones/projects 等），
    /// probe 成功后写回，卡片直接渲染，避免每次点按钮调云 API。
    #[serde(default)]
    pub probe_data: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

/// 元数据视图（不含明文，用于列表返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMeta {
    pub id: String,
    pub project: String,
    pub service: String,
    pub key_name: String,
    pub crypto_version: i32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub identity: String,
    #[serde(default)]
    pub probe_data: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl Secret {
    /// 转为无明文的元数据视图
    pub fn to_meta(&self) -> SecretMeta {
        SecretMeta {
            id: self.id.clone(),
            project: self.project.clone(),
            service: self.service.clone(),
            key_name: self.key_name.clone(),
            crypto_version: self.crypto_version,
            description: self.description.clone(),
            tags: self.tags.clone(),
            source: self.source.clone(),
            identity: self.identity.clone(),
            probe_data: self.probe_data.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

/// 创建密钥的请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSecretRequest {
    pub project: String,
    #[serde(default)]
    pub service: String,
    pub key_name: String,
    pub value: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub identity: String,
    #[serde(default)]
    pub probe_data: serde_json::Value,
}

/// 更新密钥的请求（全字段可选）
#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateSecretRequest {
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// AI 批量取值请求
#[derive(Debug, Clone, Deserialize)]
pub struct ResolveRequest {
    pub items: Vec<ResolveItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveItem {
    pub project: String,
    #[serde(default)]
    pub service: Option<String>,
    pub key_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolveResult {
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    pub key_name: String,
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// AI 批量导入请求项（upsert 语义）
#[derive(Debug, Clone, Deserialize)]
pub struct ImportItem {
    pub project: String,
    #[serde(default)]
    pub service: String,
    pub key_name: String,
    pub value: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub identity: String,
}

/// API Key（用于签发 JWT）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key_hash: String,
    pub scopes: Vec<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// 项目树节点（UI 侧栏）
#[derive(Debug, Clone, Serialize)]
pub struct ProjectNode {
    pub project: String,
    pub services: Vec<String>,
    pub count: i64,
    /// 自定义 lucide 图标名（用户设置，存 meta），无则空串
    #[serde(default)]
    pub icon: String,
}

/// 分组（collection）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub created_at: String,
}

/// 创建分组请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}
