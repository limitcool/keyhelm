//! 解析 config.yaml：递归提取含密钥特征的标量叶

use std::path::Path;

use serde_yaml::Value;

use super::Candidate;

/// 密钥特征正则（大小写不敏感）
fn is_secret_key(key: &str) -> bool {
    let upper = key.to_uppercase();
    // 含这些关键词，且不是明显非敏感（URL/ENABLED/HOST/PORT）
    let sensitive = [
        "KEY",
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "API_KEY",
        "AUTH",
        "CREDENTIAL",
        "PRIVATE",
    ];
    let nonsensitive = ["ENABLED", "HOST", "PORT", "URL", "PUBLIC", "MODE", "PATH"];
    let has_sensitive = sensitive.iter().any(|s| upper.contains(s));
    let has_nonsensitive = nonsensitive.iter().any(|s| upper.contains(s));
    has_sensitive && !has_nonsensitive
}

/// 解析一个 YAML 配置，递归提取敏感标量
pub fn parse(path: &Path, project: &str) -> anyhow::Result<Vec<Candidate>> {
    let raw = std::fs::read_to_string(path)?;
    let value: Value = serde_yaml::from_str(&raw)?;
    let mut out = Vec::new();
    walk(&value, project, path, &[], &mut out);
    Ok(out)
}

fn walk(value: &Value, project: &str, path: &Path, key_path: &[String], out: &mut Vec<Candidate>) {
    match value {
        Value::Mapping(map) => {
            for (k, v) in map {
                if let Some(key) = k.as_str() {
                    let mut kp = key_path.to_vec();
                    kp.push(key.to_string());
                    walk(v, project, path, &kp, out);
                }
            }
        }
        Value::Sequence(seq) => {
            for v in seq {
                walk(v, project, path, key_path, out);
            }
        }
        Value::String(_) | Value::Number(_) | Value::Bool(_) => {
            if let Some(key) = key_path.last() {
                if is_secret_key(key) {
                    let val = match value {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => return,
                    };
                    if !val.is_empty() {
                        out.push(Candidate {
                            project: project.to_string(),
                            service: "config.yaml".into(),
                            key_name: key.clone(),
                            value: val,
                            description: String::new(),
                            source: format!("yaml:{}:{}", path.display(), key_path.join("/")),
                        });
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_secrets() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(
            &mut f,
            b"server:\n  api_key: sk-test-123\n  public_key: ignore-me\n  token: abc\n  enabled: true\n",
        )
        .unwrap();
        let items = parse(f.path(), "proj").unwrap();
        let names: Vec<_> = items.iter().map(|c| c.key_name.as_str()).collect();
        assert!(names.contains(&"api_key"));
        assert!(names.contains(&"token"));
        assert!(!names.contains(&"public_key"));
        assert!(!names.contains(&"enabled"));
    }
}
