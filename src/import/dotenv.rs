//! 解析 .env 文件：KEY=VALUE

use std::path::Path;

use super::Candidate;

/// 解析一个 .env 文件。project 为默认项目名
pub fn parse(path: &Path, project: &str) -> anyhow::Result<Vec<Candidate>> {
    let raw = std::fs::read_to_string(path)?;
    let mut out = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim().to_string();
            if key.is_empty() {
                continue;
            }
            let mut val = val.trim().to_string();
            // 去引号
            if val.len() >= 2
                && ((val.starts_with('"') && val.ends_with('"'))
                    || (val.starts_with('\'') && val.ends_with('\'')))
            {
                val = val[1..val.len() - 1].to_string();
            }
            // 去掉行尾注释（引号外的 #）
            if !val.starts_with('#') {
                if let Some(idx) = val.find(" #") {
                    val.truncate(idx);
                }
            }
            if val.is_empty() {
                continue;
            }
            let source = format!("dotenv:{}:{}", path.display(), key);
            out.push(Candidate {
                project: project.to_string(),
                service: String::new(),
                key_name: key,
                value: val,
                description: String::new(),
                source,
            });
        } else {
            tracing::debug!("第 {} 行无 '=' 跳过: {}", i + 1, line);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_basic() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"KEY1=value1\nKEY2=\"quoted value\"\n# comment\nEMPTY=\nKEY3='single'\n")
            .unwrap();
        let items = parse(f.path(), "proj").unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].key_name, "KEY1");
        assert_eq!(items[0].value, "value1");
        assert_eq!(items[1].value, "quoted value");
        assert_eq!(items[2].value, "single");
    }
}
