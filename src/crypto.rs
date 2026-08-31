//! 加密层：主密钥加载 + AES-256-GCM 加解密
//!
//! 密文格式：base64( 12字节nonce ‖ aes-gcm输出 )，其中 aes-gcm 的 encrypt() 输出
//! 已包含 16 字节认证 tag，解密失败即 AEAD 校验失败，自带防篡改。

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use std::path::Path;
use std::sync::Arc;

use crate::config::CryptoConfig;

/// 主密钥：32 字节，进程内只以 Arc 持有
pub type MasterKey = Arc<[u8; 32]>;

/// 从配置加载主密钥：优先内联值，其次 env，最后文件
pub fn load_master_key(cfg: &CryptoConfig) -> anyhow::Result<MasterKey> {
    if !cfg.master_key_value.trim().is_empty() {
        return Ok(Arc::new(parse_key(&cfg.master_key_value)?));
    }
    if let Ok(raw) = std::env::var(&cfg.master_key_env) {
        if !raw.trim().is_empty() {
            return Ok(Arc::new(parse_key(&raw)?));
        }
    }
    if cfg.master_key_file.exists() {
        let raw = std::fs::read(&cfg.master_key_file)?;
        let key = parse_key_bytes(&raw)?;
        return Ok(Arc::new(key));
    }
    anyhow::bail!(
        "未找到主密钥：请设置 env {} 或文件 {}（可用 `keyhelm gen-key` 生成）",
        cfg.master_key_env,
        cfg.master_key_file.display()
    )
}

/// 解析密钥字符串：优先按 32 字节 raw 处理；否则尝试 hex；否则 base64
pub fn parse_key(input: &str) -> anyhow::Result<[u8; 32]> {
    let trimmed = input.trim();
    // 去掉可能的 0x / base64 前缀
    let body = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    // raw: 恰好 32 字节且可打印
    let bytes = body.as_bytes();
    if bytes.len() == 32 {
        let mut k = [0u8; 32];
        k.copy_from_slice(bytes);
        return Ok(k);
    }
    // hex: 64 个 hex 字符
    if bytes.len() == 64 && bytes.iter().all(|b| b.is_ascii_hexdigit()) {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = u8::from_str_radix(&body[i * 2..i * 2 + 2], 16)?;
        }
        return Ok(k);
    }
    // base64
    if let Ok(decoded) = B64.decode(body) {
        if decoded.len() == 32 {
            let mut k = [0u8; 32];
            k.copy_from_slice(&decoded);
            return Ok(k);
        }
    }
    anyhow::bail!("无法解析主密钥：需为 32 字节 raw / 64 位 hex / base64")
}

/// 解析密钥字节（文件内容）
fn parse_key_bytes(bytes: &[u8]) -> anyhow::Result<[u8; 32]> {
    if bytes.len() == 32 {
        let mut k = [0u8; 32];
        k.copy_from_slice(bytes);
        return Ok(k);
    }
    // 可能是文本（hex/base64），去空白后解析
    let s = String::from_utf8_lossy(bytes);
    parse_key(&s)
}

/// 生成一个新的随机主密钥（返回 hex 字符串便于 env 使用）
pub fn generate_key_hex() -> String {
    let mut key = [0u8; 32];
    rand::rng().fill_bytes(&mut key);
    key.iter().map(|b| format!("{b:02x}")).collect()
}

/// 用主密钥加密明文 → base64( nonce ‖ ciphertext+tag )
pub fn encrypt(master_key: &MasterKey, plaintext: &str) -> anyhow::Result<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master_key.as_ref()));
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("加密失败: {e:?}"))?;
    let mut blob = Vec::with_capacity(12 + ct.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ct);
    Ok(B64.encode(blob))
}

/// 用主密钥解密 → 明文。失败 = AEAD 校验失败（错钥或篡改）
pub fn decrypt(master_key: &MasterKey, encoded: &str) -> anyhow::Result<String> {
    let blob = B64
        .decode(encoded)
        .map_err(|_| anyhow::anyhow!("密文不是合法 base64"))?;
    if blob.len() < 13 {
        anyhow::bail!("密文长度非法");
    }
    let (nonce_bytes, ct) = blob.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master_key.as_ref()));
    let nonce = Nonce::from_slice(nonce_bytes);
    let pt = cipher
        .decrypt(nonce, ct)
        .map_err(|_| anyhow::anyhow!("解密失败：主密钥错误或数据被篡改"))?;
    String::from_utf8(pt).map_err(|_| anyhow::anyhow!("解密结果不是合法 UTF-8"))
}

/// 生成 master.key 文件（raw 32 字节），返回 hex 形式
pub fn generate_master_key_file(path: &Path) -> anyhow::Result<String> {
    let hex = generate_key_hex();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &hex)?;
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> MasterKey {
        Arc::new([7u8; 32])
    }

    #[test]
    fn roundtrip() {
        let key = test_key();
        let enc = encrypt(&key, "sk-ant-12345secret").unwrap();
        let dec = decrypt(&key, &enc).unwrap();
        assert_eq!(dec, "sk-ant-12345secret");
    }

    #[test]
    fn wrong_key_fails() {
        let key = test_key();
        let other = Arc::new([8u8; 32]);
        let enc = encrypt(&key, "secret-value").unwrap();
        assert!(decrypt(&other, &enc).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = test_key();
        let enc = encrypt(&key, "secret-value").unwrap();
        let blob = B64.decode(&enc).unwrap();
        let mut tampered = blob.clone();
        // 翻转密文区第一个字节
        let idx = 12;
        tampered[idx] ^= 0x01;
        let enc2 = B64.encode(tampered);
        assert!(decrypt(&key, &enc2).is_err());
    }

    #[test]
    fn tampered_nonce_fails() {
        let key = test_key();
        let enc = encrypt(&key, "secret-value").unwrap();
        let blob = B64.decode(&enc).unwrap();
        let mut tampered = blob.clone();
        tampered[0] ^= 0x01;
        let enc2 = B64.encode(tampered);
        assert!(decrypt(&key, &enc2).is_err());
    }

    #[test]
    fn parse_variants() {
        // hex
        let hex: String = (0..32).map(|i| format!("{i:02x}")).collect();
        let k = parse_key(&hex).unwrap();
        assert_eq!(k[0], 0x00);
        assert_eq!(k[31], 0x1f);
        // base64 of 32 zero bytes
        let b64 = B64.encode([0u8; 32]);
        let k2 = parse_key(&b64).unwrap();
        assert_eq!(k2, [0u8; 32]);
    }

    #[test]
    fn key_gen_and_file() {
        let hex = generate_key_hex();
        assert_eq!(hex.len(), 64);
        let key = parse_key(&hex).unwrap();
        assert_eq!(key.len(), 32);
    }
}
