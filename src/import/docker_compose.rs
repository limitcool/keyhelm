//! 解析 docker-compose.yaml：提取 services.*.environment（map + list 形式）和 env_file

use std::path::Path;

use serde_yaml::Value;

use super::Candidate;

/// 解析一个 compose 文件，返回候选密钥
pub fn parse(path: &Path, project: &str) -> anyhow::Result<Vec<Candidate>> {
    let raw = std::fs::read_to_string(path)?;
    let value: Value = serde_yaml::from_str(&raw)?;

    let services = value
        .get("services")
        .ok_or_else(|| anyhow::anyhow!("无 services 段"))?;

    let mut out = Vec::new();
    if let Value::Mapping(map) = services {
        for (svc_key, svc_val) in map {
            let service_name = svc_key.as_str().unwrap_or("unknown").to_string();
            let env = svc_val.get("environment");
            if let Some(env) = env {
                extract_environment(env, project, &service_name, path, &mut out);
            }
            // env_file
            if let Some(ef) = svc_val.get("env_file") {
                if let Some(files) = ef.as_sequence() {
                    for f in files {
                        if let Some(s) = f.as_str() {
                            let file_path = path.parent().unwrap_or(Path::new(".")).join(s);
                            if let Ok(items) = super::dotenv::parse(&file_path, project) {
                                for mut item in items {
                                    item.service = service_name.clone();
                                    item.source = format!("env_file:{}", file_path.display());
                                    out.push(item);
                                }
                            }
                        }
                    }
                } else if let Some(s) = ef.as_str() {
                    let file_path = path.parent().unwrap_or(Path::new(".")).join(s);
                    if let Ok(items) = super::dotenv::parse(&file_path, project) {
                        for mut item in items {
                            item.service = service_name.clone();
                            item.source = format!("env_file:{}", file_path.display());
                            out.push(item);
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// 提取 environment 段（map 或 list）
fn extract_environment(
    env: &Value,
    project: &str,
    service: &str,
    path: &Path,
    out: &mut Vec<Candidate>,
) {
    match env {
        Value::Mapping(map) => {
            for (k, v) in map {
                let key = k.as_str().unwrap_or_default().to_string();
                // 跳过含 file/suffix 的引用形式（如 KEY_FILE）
                if key.ends_with("_FILE") || key.ends_with("_PATH") {
                    continue;
                }
                let val = match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => continue, // 嵌套结构跳过
                };
                if val.is_empty() {
                    continue;
                }
                let source = format!("compose:{}:env:{}", path.display(), key);
                out.push(Candidate {
                    project: project.to_string(),
                    service: service.to_string(),
                    key_name: key,
                    value: val,
                    description: String::new(),
                    source,
                });
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                if let Some(s) = item.as_str() {
                    if let Some((key, val)) = s.split_once('=') {
                        let key = key.trim().to_string();
                        if key.ends_with("_FILE") || key.ends_with("_PATH") {
                            continue;
                        }
                        let val = val.trim().to_string();
                        if val.is_empty() {
                            continue;
                        }
                        let source = format!("compose:{}:env:{}", path.display(), key);
                        out.push(Candidate {
                            project: project.to_string(),
                            service: service.to_string(),
                            key_name: key,
                            value: val,
                            description: String::new(),
                            source,
                        });
                    }
                }
            }
        }
        _ => {}
    }
}
