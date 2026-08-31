// Keyhelm REST API 客户端（与 src/api/*.rs 对应）

export interface Secret {
  id: string
  project: string
  service: string | null
  key_name: string
  description: string | null
  tags: string[]
  source: string | null
  updated_at: string
  value?: string // 仅 ?reveal=1 或 reveal 端点返回
  created_at?: string
  /** 账号身份（云厂商探测后写回，如 RAM 用户 / 腾讯 AccountId / GCP client_email） */
  identity?: string
  /** 云厂商探测到的可访问资源/权限（非机密 JSON：buckets/policies/zones/projects 等） */
  probe_data?: Record<string, unknown>
}

export interface Project {
  project: string
  services: string[]
  count: number
  /** 自定义 lucide 图标名（用户设置，后端 meta 持久化），无则空串 */
  icon?: string
}

export interface ListResponse {
  items: Secret[]
  total: number
  page: number
  page_size: number
}

export interface ProjectTreeResponse {
  projects: Project[]
}

export interface ImportResult {
  created: number
  updated: number
  errors?: string[]
}

export interface CloudVerifyResult {
  provider: string
  valid: boolean
  account_id?: string
  account?: string
  accountName?: string
  arn?: string
  principal_id?: string
  email?: string
  token_id?: string
  status?: string
  error?: string
}

// 预置云厂商（侧栏固定区，与后端约定）。
// supported：后端 cloud.rs 是否实现了 verify/probe。未实现的显示为「未接入」，
// 避免用户点了永远报「不支持的云厂商」。
export const CLOUD_PROVIDERS = [
  { name: "aliyun", label: "阿里云", supported: true },
  { name: "tencent", label: "腾讯云", supported: true },
  { name: "cloudflare", label: "Cloudflare", supported: true },
  { name: "google-cloud", label: "Google Cloud", supported: true },
  { name: "aws", label: "AWS", supported: false },
  { name: "azure", label: "Azure", supported: false },
  { name: "digitalocean", label: "DigitalOcean", supported: false },
]

const BASE = "/api/v1"

// 统一请求：401 → 抛未登录；解析 JSON 错误
export async function api<T = unknown>(
  path: string,
  opts: RequestInit = {}
): Promise<T> {
  const headers: Record<string, string> = {
    ...(opts.headers as Record<string, string> | undefined),
  }
  if (opts.body && !(opts.body instanceof FormData)) {
    headers["Content-Type"] = "application/json"
  }
  const res = await fetch(`${BASE}${path}`, { ...opts, headers })
  if (res.status === 401) {
    throw new ApiError("未登录", 401)
  }
  if (!res.ok) {
    let msg = res.statusText
    try {
      const j = await res.json()
      msg = j.error || j.message || msg
    } catch {
      /* 非 JSON 错误体 */
    }
    throw new ApiError(msg, res.status)
  }
  if (res.status === 204) return null as T
  return res.json() as Promise<T>
}

export class ApiError extends Error {
  status: number
  constructor(message: string, status: number) {
    super(message)
    this.status = status
  }
}

// ---- 登录 / 登出（cookie session） ----
export async function login(username: string, password: string) {
  const res = await fetch("/ui/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username, password }),
  })
  if (!res.ok) {
    let msg = "登录失败"
    try {
      const j = await res.json()
      msg = j.error || msg
    } catch {
      /* ignore */
    }
    throw new ApiError(msg, res.status)
  }
}

export async function logout() {
  await fetch("/ui/logout", { method: "POST" })
}

// ---- 项目树 ----
export async function getProjects(): Promise<Project[]> {
  const data = await api<ProjectTreeResponse>("/projects")
  return data.projects || []
}

/** 设置项目自定义 lucide 图标名（空串清除） */
export function setProjectIcon(project: string, icon: string) {
  return api(`/projects/${encodeURIComponent(project)}/icon`, {
    method: "PUT",
    body: JSON.stringify({ icon }),
  })
}

// ---- 密钥列表（reveal=1 直接带明文） ----
export interface ListParams {
  project?: string | null
  service?: string | null
  q?: string
  page?: number
  page_size?: number
  reveal?: boolean
}

export async function listSecrets(params: ListParams = {}): Promise<ListResponse> {
  const sp = new URLSearchParams()
  if (params.project) sp.set("project", params.project)
  if (params.service) sp.set("service", params.service)
  if (params.q) sp.set("q", params.q)
  sp.set("page", String(params.page ?? 0))
  sp.set("page_size", String(params.page_size ?? 50))
  if (params.reveal) sp.set("reveal", "1")
  return api<ListResponse>(`/secrets?${sp}`)
}

// ---- 密钥 CRUD ----
export interface SecretPayload {
  project: string
  service?: string
  key_name: string
  value?: string
  description?: string
  tags?: string[]
}

export function createSecret(payload: SecretPayload) {
  return api("/secrets", { method: "POST", body: JSON.stringify(payload) })
}

export function updateSecret(id: string, payload: Partial<SecretPayload>) {
  return api(`/secrets/${id}`, { method: "PUT", body: JSON.stringify(payload) })
}

export function deleteSecret(id: string) {
  return api(`/secrets/${id}`, { method: "DELETE" })
}

export async function revealSecret(id: string): Promise<string> {
  const data = await api<{ value: string }>(`/secrets/${id}/value`)
  return data.value
}

// ---- 批量导入 ----
export function importSecrets(items: SecretPayload[]): Promise<ImportResult> {
  return api("/import", { method: "POST", body: JSON.stringify(items) })
}

// ---- 云厂商验证 / 探测 ----
export function cloudVerify(provider: string, project?: string) {
  const q = project ? `?project=${encodeURIComponent(project)}` : ""
  return api<CloudVerifyResult>(`/cloud/${provider}/verify${q}`, { method: "POST" })
}

export function cloudProbe(provider: string, project?: string) {
  const q = project ? `?project=${encodeURIComponent(project)}` : ""
  return api(`/cloud/${provider}/probe${q}`, { method: "POST" })
}

// ---- 导出 .env（拼剪贴板文本） ----
export async function exportEnv(project: string): Promise<string> {
  const data = await listSecrets({ project, page_size: 500, reveal: true })
  const lines: string[] = []
  for (const s of data.items || []) {
    if (s.value != null) lines.push(`${s.key_name}=${s.value}`)
  }
  return lines.join("\n")
}
