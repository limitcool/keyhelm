# ===== 阶段 1：构建前端 Web UI =====
# vite 的 outDir 是相对前端目录的 ../../src/ui/static，
# 所以前端源码必须放在 /workspace/web/keyhelm-web/ 下，
# 构建产物才恰好落到 /workspace/src/ui/static/，与仓库结构一致。
FROM node:20-alpine AS frontend
WORKDIR /workspace/web/keyhelm-web
COPY web/keyhelm-web/package.json web/keyhelm-web/pnpm-lock.yaml web/keyhelm-web/pnpm-workspace.yaml ./
RUN corepack enable && corepack prepare pnpm@latest --activate \
    && pnpm install --frozen-lockfile
COPY web/keyhelm-web/ ./
RUN pnpm build

# ===== 阶段 2：编译 Rust 后端 =====
FROM rust:1-bookworm AS builder
# bundled sqlite 需要 C 编译器；reqwest rustls 需要 libssl-dev
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
# 先拷贝清单与源码，利用 Docker 层缓存
COPY Cargo.toml Cargo.lock build.rs ./
COPY src/ ./src/
# 前端构建产物（阶段 1 已输出到 /workspace/src/ui/static/）
COPY --from=frontend /workspace/src/ui/static/ ./src/ui/static/
RUN cargo build --release

# ===== 阶段 3：运行时 =====
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 keyhelm
WORKDIR /app
COPY --from=builder /workspace/target/release/keyhelm /usr/local/bin/keyhelm
RUN mkdir -p /data && chown keyhelm:keyhelm /data
USER keyhelm
EXPOSE 8080
VOLUME ["/data"]
# 默认走 sqlite，数据落 /data 卷；可用 KEYHELM_* 覆盖
ENV KEYHELM_BIND_ADDR=0.0.0.0:8080 \
    KEYHELM_DB_KIND=sqlite \
    KEYHELM_DB_PATH=/data/keyhelm.db
ENTRYPOINT ["keyhelm"]
CMD ["serve"]
