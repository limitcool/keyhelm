// build.rs — 让 cargo 跟踪嵌入的 Web 前端产物变化
// vite 构建产物输出到 src/ui/static/，rust-embed 在编译期嵌入。
// 这里声明这些文件为构建依赖，改动时触发重新编译。

fn main() {
    println!("cargo:rerun-if-changed=src/ui/static");
}
