//! 嵌入式 Web UI（rust-embed，零构建 vanilla JS）

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "src/ui/static/"]
pub struct UiAssets;

impl UiAssets {
    /// 读取 index.html
    pub fn index_html() -> anyhow::Result<String> {
        let file = Self::get("index.html").ok_or_else(|| anyhow::anyhow!("index.html 未嵌入"))?;
        Ok(String::from_utf8_lossy(file.data.as_ref()).into_owned())
    }
}
