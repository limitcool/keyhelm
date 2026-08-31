---
name: keyhelm
description: >-
  使用 Keyhelm 密钥配置中心（Secret Config Center）的指南。当用户需要读取/查询/写入/管理
  服务器上的 API 密钥、云厂商密钥、AI 服务密钥，或者让 AI 程序化访问统一密钥库时使用本 skill。
  也用于：AI 需要某服务的密钥但不知道值、需要把新密钥存进去、需要验证某云厂商 key 是否有效、
  需要批量取一批密钥给脚本/部署用。覆盖：认证换 JWT、列出/搜索密钥、取明文、批量取值(resolve)、
  批量写入(import upsert)、云厂商验证(阿里云/腾讯云/Cloudflare/Google)、分组、权限 scope、
  项目自定义图标。
---

# Keyhelm — 密钥配置中心使用指南

Keyhelm 是聚合管理散落密钥的统一配置中心：服务器上各个服务（docker-compose）、云厂商
（阿里云/腾讯云/Cloudflare/Google Cloud）、AI 服务（newapi/grok2api/chatgpt2api 等）的
API key 都集中存在这里。密钥用 AES-256-GCM 加密落库，AI 通过 REST API 程序化读取/写入。

## 触发时机

- 需要某个服务的 API key / token / 密码，但不知道具体值
- 要把一个新密钥存进配置中心（比如部署新服务、配了新云账号）
- 要批量取/写一组密钥（如导出 .env、给脚本配置环境）
- 要验证某个云厂商的 key 是否有效、能访问哪些资源
- 用户提到"去配置中心拿 key""查一下 xxx 的密钥""把密钥存起来"

## 前置

- 服务地址与凭据：默认 `http://<host>:8080`。base URL 可用 env `KEYHELM_URL` 指定。
- 你需要一个 **API key**（形如 `kh_...`）。若没有，找管理员创建（admin scope）。
- API key 换取 JWT 后，所有请求带 `Authorization: Bearer <jwt>`。

## 认证：API key → JWT

```bash
# 用 API key 换 JWT（token 有有效期，用完即弃）
curl -X POST "${KEYHELM_URL:-http://127.0.0.1:8080}/api/v1/token" \
  -H "Content-Type: application/json" \
  -d '{"api_key":"kh_YOUR_API_KEY"}'
# → {"access_token":"eyJ...","token_type":"Bearer","expires_in":3600}
```

得到 `access_token` 后，所有 API 调用带：
```
Authorization: Bearer eyJ...
```

### scope 权限

- `read` — 看列表、取明文、resolve
- `write` — 创建/更新/删除密钥、import
- `admin` — 管理 API key、分组

你的 key 有什么 scope 就用什么操作；只读 key 写操作会返回 403。

## 读密钥

### 1. 列出/搜索（默认不含明文）

```bash
# 全部
curl -H "Authorization: Bearer $JWT" "${BASE}/api/v1/secrets"
# 按项目/服务/关键词/标签过滤 + 分页
curl -H "Authorization: Bearer $JWT" \
  "${BASE}/api/v1/secrets?project=aliyun&q=ACCESS&tag=cloud&page=0&page_size=50"
```

响应 `items[]` 每项：`{id, project, service, key_name, description, tags, source, updated_at, ...}`。
**注意：列表默认不含 `value`（明文），这是安全设计。**

### 2. 取单个明文（最常用）

```bash
# 方式 A：按 project + key_name 快捷取值（忽略 service 维度）
curl -H "Authorization: Bearer $JWT" "${BASE}/api/v1/values/${project}/${key_name}"
# → {"value":"sk-ant-xxx","key_name":"...","project":"..."}

# 方式 B：按 id reveal（会记审计日志）
curl -H "Authorization: Bearer $JWT" "${BASE}/api/v1/secrets/${id}/value"
```

### 3. 列表直接带明文（web UI 同款）

```bash
curl -H "Authorization: Bearer $JWT" "${BASE}/api/v1/secrets?reveal=1"
# items[] 里多了 value 字段（明文）
```

### 4. AI 批量取值（resolve，一次取多个）

```bash
curl -X POST -H "Authorization: Bearer $JWT" -H "Content-Type: application/json" \
  "${BASE}/api/v1/resolve" -d '{
    "items": [
      {"project":"newapi", "key_name":"ANTHROPIC_API_KEY"},
      {"project":"cloudflare", "key_name":"CLOUDFLARE_API_TOKEN"},
      {"project":"aliyun", "service":"oss", "key_name":"ALIYUN_ACCESS_KEY_ID"}
    ]
  }'
# → {"results":[{"project":"newapi","key_name":"ANTHROPIC_API_KEY","value":"sk-..."}, ...]}
# 找不到的项返回 {"error":"not_found"}（不会整体失败）
```

### 5. 项目树（了解有哪些项目/服务）

```bash
curl -H "Authorization: Bearer $JWT" "${BASE}/api/v1/projects"
# → {"projects":[{"project":"newapi","services":["docker-compose"],"count":5,"icon":"Zap"}, ...]}
# icon 是用户自定义的 lucide 图标名（可选，空串表示未设置）
```

### 6. 项目图标（自定义 lucide 图标，开源友好）

每个项目可以设置/清除一个 lucide 图标名（存后端 meta，UI 侧栏/卡片会用）。
可用于把新项目做得可读，不依赖前端硬编码映射。

```bash
# 设置图标（write scope）
curl -X PUT -H "Authorization: Bearer $JWT" -H "Content-Type: application/json" \
  "${BASE}/api/v1/projects/${project}/icon" -d '{"icon":"Rocket"}'
# → {"project":"newapi","icon":"Rocket"}

# 清除图标（空串）
curl -X PUT -H "Authorization: Bearer $JWT" -H "Content-Type: application/json" \
  "${BASE}/api/v1/projects/${project}/icon" -d '{"icon":""}'
```

- `icon` 只允许字母/数字/连字符（lucide 图标名格式）。
- 项目树 `GET /api/v1/projects` 会带上每个项目的 `icon` 字段。
- 常见图标名：`Bot` `Zap` `Sparkles` `Server` `Globe` `Database` `Lock` `Terminal` `Rocket` `Shield` 等。
- 从 Web UI 新建密钥时，输入一个**不存在**的项目名会弹出图标选择器，保存后自动调用此 API。

## 写密钥

### 创建单个

```bash
curl -X POST -H "Authorization: Bearer $JWT" -H "Content-Type: application/json" \
  "${BASE}/api/v1/secrets" -d '{
    "project":"newapi", "service":"docker-compose",
    "key_name":"ANTHROPIC_API_KEY", "value":"sk-ant-secret",
    "description":"Claude API key", "tags":["ai","anthropic"]
  }'
# 201 → {"secret":{...}} ；重复 → 409
```

### AI 批量写入（import，upsert 语义，推荐）

```bash
curl -X POST -H "Authorization: Bearer $JWT" -H "Content-Type: application/json" \
  "${BASE}/api/v1/import" -d '[
    {"project":"grok2api","service":"config","key_name":"XAI_API_KEY","value":"sk-xai-1","tags":["ai"]},
    {"project":"cloudflare","service":"api","key_name":"CLOUDFLARE_API_TOKEN","value":"cf-token-xyz"}
  ]'
# → {"created":1,"updated":1}   已存在则更新，不存在则新建（幂等，可重复执行）
```

### 更新 / 删除

```bash
# 更新（只发要改的字段，value 可选）
curl -X PUT -H "Authorization: Bearer $JWT" -H "Content-Type: application/json" \
  "${BASE}/api/v1/secrets/${id}" -d '{"value":"new-secret","description":"rotated"}'
# 删除
curl -X DELETE -H "Authorization: Bearer $JWT" "${BASE}/api/v1/secrets/${id}"
# 204
```

## 云厂商集成：验证 key + 探测权限

四个 provider 支持直接调云 API 验证 key 有效性并探测可访问资源。
密钥放在对应 project 下：`aliyun` / `tencent` / `cloudflare` / `google-cloud`。

| provider | 需要哪些 key_name |
|---|---|
| aliyun | `ALIYUN_ACCESS_KEY_ID` + `ALIYUN_ACCESS_KEY_SECRET` |
| tencent | `TENCENT_SECRET_ID` + `TENCENT_SECRET_KEY` |
| cloudflare | `CLOUDFLARE_API_TOKEN` |
| google-cloud | `GOOGLE_SERVICE_ACCOUNT_KEY`（服务账号 JSON） |

```bash
# 验证 key 有效性（返回账号身份）
curl -X POST -H "Authorization: Bearer $JWT" "${BASE}/api/v1/cloud/${provider}/verify"
# 探测可访问资源（Cloudflare 返回 accounts+zones，Google 返回 projects 等）
curl -X POST -H "Authorization: Bearer $JWT" "${BASE}/api/v1/cloud/${provider}/probe"
# 若 project 名与 provider 不同，用 ?project=<实际名>
```

成功 → `{"provider":"aliyun","valid":true,"account_id":"..."}`；key 无效 → `{"error":"..."}`。

## 分组（collections，可选）

用于给密钥打业务分组（如"给 AI 用的一组 key"）。

```bash
# 建组
curl -X POST -H "Authorization: Bearer $JWT" -H "Content-Type: application/json" \
  "${BASE}/api/v1/collections" -d '{"name":"ai-keys","description":"AI 用的"}'
# 加/移/列
curl -X PUT   -H "Authorization: Bearer $JWT" "${BASE}/api/v1/collections/${cid}/items/${sid}"   # 加入
curl -X DELETE -H "Authorization: Bearer $JWT" "${BASE}/api/v1/collections/${cid}/items/${sid}"  # 移出
curl -H "Authorization: Bearer $JWT" "${BASE}/api/v1/collections/${cid}/items"                    # 列组内密钥
```

## 规范与安全约定

- **project 语义**：服务名（newapi、grok2api…）或云厂商名（aliyun、tencent、cloudflare、google-cloud）。
- **key_name 用大写 env 风格**：`ANTHROPIC_API_KEY`、`CLOUDFLARE_API_TOKEN`。
- **绝不在日志/回显里打印密钥明文**。取到 value 后只在需要的地方使用。
- **列表默认不含明文**——需要明文显式用 `/values/...`、`/secrets/{id}/value` 或 `?reveal=1`。
- **写操作用 write scope**；批量写入优先用 `/import`（幂等 upsert），比逐条 create 高效且不报 409。
- **reveal 和 resolve 会记审计日志**——正常行为，别慌。
- 删除是不可逆的，删除前先确认 `{project}/{key_name}`。

## 快速上手例子

> 用户："帮我把 newapi 的 ANTHROPIC_API_KEY 拿出来，顺便验证下 cloudflare token"

```bash
JWT=$(curl -s -X POST "$BASE/api/v1/token" -H "Content-Type: application/json" \
  -d "{\"api_key\":\"$KH_API_KEY\"}" | jq -r .access_token)

# 取 newapi 的 anthropic key
curl -s -H "Authorization: Bearer $JWT" "$BASE/api/v1/values/newapi/ANTHROPIC_API_KEY"

# 验证 cloudflare token
curl -s -X POST -H "Authorization: Bearer $JWT" "$BASE/api/v1/cloud/cloudflare/verify"
```

> 用户："我把新的 xai key 存进去"

```bash
curl -s -X POST -H "Authorization: Bearer $JWT" -H "Content-Type: application/json" \
  "$BASE/api/v1/import" -d '[{"project":"grok2api","key_name":"XAI_API_KEY","value":"sk-xai-new"}]'
```
