import { useCallback, useEffect, useState } from "react"
import {
  Search,
  Plus,
  LogOut,
  Upload,
  Download,
  X,
  Loader2,
  RefreshCw,
  ShieldCheck,
} from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { Badge } from "@/components/ui/badge"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { Sidebar } from "@/components/Sidebar"
import { SecretCard } from "@/components/SecretCard"
import { CredentialCard } from "@/components/CredentialCard"
import { SecretDialog } from "@/components/SecretDialog"
import { ImportDialog } from "@/components/ImportDialog"
import { ProviderIcon } from "@/components/ProviderIcon"
import {
  cloudProbe,
  cloudVerify,
  createSecret,
  deleteSecret,
  exportEnv,
  getProjects,
  listSecrets,
  logout,
  updateSecret,
  CLOUD_PROVIDERS,
  type Project,
  type Secret,
  type SecretPayload,
} from "@/lib/api"

interface DashboardProps {
  onLoggedOut: () => void
}

const PAGE_SIZE = 24 // 卡片网格一页 24 张

function copyToClipboard(text: string, label: string) {
  if (!text) return
  navigator.clipboard
    .writeText(text)
    .then(() => toast.success(`已复制 ${label}`))
    .catch(() => {
      window.prompt("复制失败，请手动复制：", text)
    })
}

// 凭据组：同一个 (project, service) 下的关联密钥（如 ID + KEY）合并为一张卡
type SecretGroup = { project: string; service: string | null; secrets: Secret[] }

function groupSecrets(items: Secret[]): SecretGroup[] {
  const map = new Map<string, SecretGroup>()
  for (const s of items) {
    const key = `${s.project}::${s.service ?? ""}`
    const g = map.get(key)
    if (g) g.secrets.push(s)
    else map.set(key, { project: s.project, service: s.service, secrets: [s] })
  }
  return [...map.values()]
}

const PROBE_LABELS: Record<string, string> = {
  buckets: "存储桶",
  policies: "RAM 权限",
  accounts: "账号",
  zones: "域名",
  projects: "项目",
  regions: "地域",
  errors: "错误",
}

// 把探测结果渲染成友好的分组列表，而不是原始 JSON
function ProbeResult({ data }: { data: unknown }) {
  const d = data as Record<string, unknown>
  const arrayEntries = Object.entries(d).filter(
    ([, v]) => Array.isArray(v) && v.length > 0
  )
  const scalarEntries = Object.entries(d).filter(
    ([, v]) => v != null && typeof v !== "object"
  )

  const errors = (d.errors as unknown[] | undefined) ?? []

  return (
    <div className="flex flex-col gap-2">
      {arrayEntries
        .filter(([k]) => k !== "errors")
        .map(([k, v]) => (
          <div key={k} className="flex flex-col gap-1">
            <span className="text-xs font-medium text-muted-foreground">
              {PROBE_LABELS[k] ?? k}
            </span>
            <div className="flex flex-wrap gap-1">
              {(v as unknown[]).map((item, i) => {
                const o =
                  typeof item === "object" && item !== null
                    ? (item as Record<string, unknown>)
                    : null
                const label = o ? (o.name ?? o.id ?? o.accountId) : item
                const sub = o ? (o.type ? String(o.type) : null) : null
                return (
                  <Badge key={i} variant="outline" className="font-mono font-normal">
                    {String(label ?? item ?? "")}
                    {sub && (
                      <span className="ml-1 text-[10px] text-muted-foreground">
                        · {sub}
                      </span>
                    )}
                  </Badge>
                )
              })}
            </div>
          </div>
        ))}
      {scalarEntries.length > 0 && (
        <div className="flex flex-wrap gap-x-4 gap-y-1">
          {scalarEntries.map(([k, v]) => (
            <span key={k} className="text-xs text-muted-foreground">
              {PROBE_LABELS[k] ?? k}:{" "}
              <span className="font-mono text-foreground">{String(v)}</span>
            </span>
          ))}
        </div>
      )}
      {errors.length > 0 && (
        <div className="flex flex-col gap-0.5 rounded-md border border-destructive/30 bg-destructive/5 px-2 py-1.5">
          <span className="text-xs font-medium text-destructive">
            部分探测失败
          </span>
          {errors.map((e, i) => (
            <span key={i} className="text-[11px] text-destructive/80">
              {String(e)}
            </span>
          ))}
        </div>
      )}
      {arrayEntries.length === 0 && scalarEntries.length === 0 && errors.length === 0 && (
        <p className="text-xs text-muted-foreground">没有可访问的资源</p>
      )}
    </div>
  )
}

export function Dashboard({ onLoggedOut }: DashboardProps) {
  const [projects, setProjects] = useState<Project[]>([])
  const [selectedProject, setSelectedProject] = useState<string | null>(null)
  const [selectedService, setSelectedService] = useState<string | null>(null)
  const [q, setQ] = useState("")
  const [searchInput, setSearchInput] = useState("")
  const [secrets, setSecrets] = useState<Secret[]>([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(0)
  const [loading, setLoading] = useState(true)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editing, setEditing] = useState<Secret | null>(null)

  // ---- 数据加载 ----
  const loadProjects = useCallback(async () => {
    try {
      setProjects(await getProjects())
    } catch (e) {
      if ((e as { status?: number }).status === 401) {
        onLoggedOut()
        return
      }
      toast.error("加载项目树失败")
    }
  }, [onLoggedOut])

  const loadList = useCallback(async () => {
    setLoading(true)
    try {
      const data = await listSecrets({
        project: selectedProject,
        service: selectedService,
        q,
        page,
        page_size: PAGE_SIZE,
        reveal: true,
      })
      setSecrets(data.items ?? [])
      setTotal(data.total ?? 0)
    } catch (e) {
      if ((e as { status?: number }).status === 401) {
        onLoggedOut()
        return
      }
      toast.error("加载密钥失败")
    } finally {
      setLoading(false)
    }
  }, [selectedProject, selectedService, q, page, onLoggedOut])

  useEffect(() => {
    loadProjects()
  }, [loadProjects])

  useEffect(() => {
    loadList()
  }, [loadList])

  // ---- 筛选交互 ----
  function doSearch(e: React.FormEvent) {
    e.preventDefault()
    setQ(searchInput.trim())
    setPage(0)
  }

  function clearSearch() {
    setQ("")
    setSearchInput("")
    setPage(0)
  }

  // ---- 增删改 ----
  async function handleSave(payload: SecretPayload, id?: string) {
    if (id) {
      await updateSecret(id, payload)
      toast.success("已更新")
    } else {
      await createSecret(payload)
      toast.success("已创建")
    }
    await Promise.all([loadProjects(), loadList()])
  }

  async function handleDelete(secret: Secret) {
    const ok = window.confirm(`确定删除 ${secret.project}/${secret.key_name}？`)
    if (!ok) return
    try {
      await deleteSecret(secret.id)
      toast.success("已删除")
      await Promise.all([loadProjects(), loadList()])
    } catch (e) {
      toast.error(`删除失败: ${(e as Error).message}`)
    }
  }

  // ---- 批量导入（打开 ImportDialog） ----
  const [importOpen, setImportOpen] = useState(false)
  async function handleImported() {
    await Promise.all([loadProjects(), loadList()])
  }

  // ---- 导出 .env ----
  async function handleExport() {
    if (!selectedProject) return
    try {
      const text = await exportEnv(selectedProject)
      copyToClipboard(text, `${selectedProject} .env`)
    } catch (e) {
      toast.error(`导出失败: ${(e as Error).message}`)
    }
  }

  // ---- 云厂商面板（验证 / 探测） ----
  type VerifyStatus =
    | { status: "idle" }
    | { status: "loading" }
    | { status: "ok"; detail?: string }
    | { status: "error"; detail?: string }
  const [verifyResults, setVerifyResults] = useState<Record<string, VerifyStatus>>({})
  const [probeData, setProbeData] = useState<Record<string, { data?: unknown; error?: string; loading?: boolean }>>({})
  const [cloudOpen, setCloudOpen] = useState(false)
  const [cloudTab, setCloudTab] = useState("verify")

  const supportedProviders = CLOUD_PROVIDERS.filter((c) => c.supported)
  const providerLabel = (name: string) =>
    CLOUD_PROVIDERS.find((c) => c.name === name)?.label ?? name

  async function verifyProvider(provider: string) {
    setVerifyResults((r) => ({ ...r, [provider]: { status: "loading" } }))
    try {
      const res = await cloudVerify(provider, provider)
      const detail =
        res.account_id || res.account || res.accountName || res.email || "有效"
      setVerifyResults((r) => ({ ...r, [provider]: { status: "ok", detail } }))
      toast.success(`${providerLabel(provider)} key 有效`)
    } catch (e) {
      setVerifyResults((r) => ({
        ...r,
        [provider]: { status: "error", detail: (e as Error).message },
      }))
    }
  }

  async function probeProvider(provider: string) {
    setProbeData((d) => ({ ...d, [provider]: { loading: true } }))
    try {
      const res = await cloudProbe(provider, provider)
      setProbeData((d) => ({ ...d, [provider]: { data: res } }))
      // 探针结果已写回 DB，刷新列表让卡片直接展示资源/权限
      await loadList()
    } catch (e) {
      setProbeData((d) => ({ ...d, [provider]: { error: (e as Error).message } }))
    }
  }

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))

  return (
    <div className="flex h-svh flex-col">
      {/* 顶栏 */}
      <header className="flex items-center gap-3 border-b px-4 py-2.5">
        <div className="flex items-center gap-2 text-lg font-semibold">
          <img
            src="/ui/logo-192.png"
            alt="Keyhelm logo"
            className="size-8 shrink-0 rounded-lg object-cover"
          />
          Keyhelm
        </div>
        <form onSubmit={doSearch} className="relative ml-2 flex-1 max-w-md">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            placeholder="搜索密钥…（回车）"
            className="pl-8 pr-8"
          />
          {searchInput && (
            <button
              type="button"
              onClick={clearSearch}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            >
              <X className="size-4" />
            </button>
          )}
        </form>
        <div className="ml-auto flex items-center gap-2">
          {selectedProject && (
            <Button variant="outline" size="sm" onClick={handleExport}>
              <Download className="size-4" />
              导出 .env
            </Button>
          )}
          <Button variant="outline" size="sm" onClick={() => setCloudOpen(true)}>
            <ShieldCheck className="size-4" />
            云厂商
          </Button>
          <Button size="sm" onClick={() => { setEditing(null); setDialogOpen(true) }}>
            <Plus className="size-4" />
            新增密钥
          </Button>
          <Button variant="outline" size="sm" onClick={() => setImportOpen(true)}>
            <Upload className="size-4" />
            批量导入
          </Button>
          <Button variant="ghost" size="icon" aria-label="登出" onClick={async () => { await logout(); onLoggedOut() }}>
            <LogOut className="size-4" />
          </Button>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* 侧栏 */}
        <aside className="w-56 shrink-0 border-r bg-muted/30">
          <Sidebar
            projects={projects}
            selectedProject={selectedProject}
            selectedService={selectedService}
            onSelect={(p, svc) => { setSelectedProject(p); setSelectedService(svc ?? null); setPage(0) }}
          />
        </aside>

        {/* 主区：卡片网格 */}
        <main className="flex-1 overflow-y-auto p-4">
          <div className="mb-3 flex items-center justify-between">
            <div className="text-sm text-muted-foreground">
              共 {total} 条密钥
              {selectedProject && <span className="ml-1">· {selectedProject}</span>}
            </div>
          </div>

          {loading ? (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
              {Array.from({ length: 8 }).map((_, i) => (
                <div key={i} className="space-y-2 rounded-lg border p-4">
                  <Skeleton className="h-4 w-2/3" />
                  <Skeleton className="h-8 w-full" />
                  <Skeleton className="h-3 w-1/2" />
                </div>
              ))}
            </div>
          ) : secrets.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-24 text-center text-muted-foreground">
              <Search className="mb-2 size-8" />
              <p>没有匹配的密钥</p>
              {q && <Button variant="link" onClick={clearSearch}>清除搜索</Button>}
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
              {groupSecrets(secrets).map((g) => {
                const isCloudCred =
                  g.secrets.length > 1 ||
                  CLOUD_PROVIDERS.some((c) => c.name === g.project)
                if (isCloudCred) {
                  return (
                    <CredentialCard
                      key={`${g.project}::${g.service ?? ""}`}
                      project={g.project}
                      projectIconName={projects.find((p) => p.project === g.project)?.icon ?? ""}
                      service={g.service}
                      identity={g.secrets.find((s) => s.identity)?.identity ?? ""}
                      probeData={g.secrets.find((s) => s.probe_data)?.probe_data}
                      secrets={g.secrets}
                      onCopy={(v, name) => copyToClipboard(v, name)}
                      onEdit={(secret) => { setEditing(secret); setDialogOpen(true) }}
                      onDelete={handleDelete}
                    />
                  )
                }
                return (
                  <SecretCard
                    key={g.secrets[0].id}
                    secret={g.secrets[0]}
                    onCopy={(v, name) => copyToClipboard(v, name)}
                    onEdit={(secret) => { setEditing(secret); setDialogOpen(true) }}
                    onDelete={handleDelete}
                  />
                )
              })}
            </div>
          )}

          {/* 分页 */}
          {totalPages > 1 && (
            <div className="mt-6 flex items-center justify-center gap-2">
              <Button variant="outline" size="sm" disabled={page === 0} onClick={() => setPage((p) => Math.max(0, p - 1))}>
                上一页
              </Button>
              <span className="text-sm text-muted-foreground">
                {page + 1} / {totalPages}
              </span>
              <Button variant="outline" size="sm" disabled={page >= totalPages - 1} onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}>
                下一页
              </Button>
            </div>
          )}
        </main>
      </div>

      {/* 新增/编辑 Dialog */}
      <SecretDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        secret={editing}
        projects={projects}
        defaultProject={selectedProject}
        onSave={handleSave}
      />

      <ImportDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        onImported={handleImported}
      />

      {/* 云厂商面板：验证 + 探测 */}
      <Dialog open={cloudOpen} onOpenChange={setCloudOpen}>
        <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-xl">
          <DialogHeader>
            <DialogTitle>云厂商</DialogTitle>
            <DialogDescription>验证各云厂商密钥是否有效，并探测 key 能访问哪些资源。</DialogDescription>
          </DialogHeader>

          <Tabs value={cloudTab} onValueChange={setCloudTab}>
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="verify">密钥验证</TabsTrigger>
              <TabsTrigger value="probe">资源探测</TabsTrigger>
            </TabsList>

            {/* 验证页签 */}
            <TabsContent value="verify" className="flex flex-col gap-1.5">
              {supportedProviders.map((c) => {
                const r = verifyResults[c.name] ?? { status: "idle" }
                return (
                  <div
                    key={c.name}
                    className="flex items-center justify-between gap-2 rounded-md border px-3 py-2"
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      <ProviderIcon provider={c.name} colored className="size-5 shrink-0" />
                      <span className="text-sm font-medium">{c.label}</span>
                      {r.status === "loading" && <Loader2 className="size-3.5 animate-spin text-muted-foreground" />}
                    </div>
                    <div className="flex min-w-0 items-center gap-2">
                      {r.status === "ok" && (
                        <span className="truncate text-xs text-green-600">{r.detail ?? "有效"}</span>
                      )}
                      {r.status === "error" && (
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <span className="max-w-40 truncate text-xs text-destructive">
                              {r.detail ?? "无效"}
                            </span>
                          </TooltipTrigger>
                          <TooltipContent>{r.detail}</TooltipContent>
                        </Tooltip>
                      )}
                      {r.status === "idle" && (
                        <span className="text-xs text-muted-foreground">未验证</span>
                      )}
                      <Button
                        variant="outline"
                        size="xs"
                        disabled={r.status === "loading"}
                        onClick={() => verifyProvider(c.name)}
                      >
                        <RefreshCw className="size-3" />
                        {r.status === "loading" ? "验证中" : "验证"}
                      </Button>
                    </div>
                  </div>
                )
              })}
              <p className="mt-1 text-xs text-muted-foreground">
                AWS、Azure、DigitalOcean 后端尚未接入验证，先支持四家。
              </p>
            </TabsContent>

            {/* 探测页签 */}
            <TabsContent value="probe" className="flex flex-col gap-1.5">
              {supportedProviders.map((c) => {
                const p = probeData[c.name]
                return (
                  <div key={c.name} className="flex flex-col gap-2 rounded-md border px-3 py-2">
                    <div className="flex items-center justify-between gap-2">
                      <div className="flex min-w-0 items-center gap-2">
                        <ProviderIcon provider={c.name} colored className="size-5 shrink-0" />
                        <span className="text-sm font-medium">{c.label}</span>
                      </div>
                      <Button
                        variant="outline"
                        size="xs"
                        disabled={p?.loading}
                        onClick={() => probeProvider(c.name)}
                      >
                        <RefreshCw className="size-3" />
                        {p?.loading ? "探测中" : "探测"}
                      </Button>
                    </div>
                    {p?.loading && (
                      <p className="flex items-center gap-2 text-xs text-muted-foreground">
                        <Loader2 className="size-3 animate-spin" /> 正在访问云 API…
                      </p>
                    )}
                    {p?.error && <p className="text-xs text-destructive">{p.error}</p>}
                    {p?.data ? <ProbeResult data={p.data} /> : null}
                  </div>
                )
              })}
              <p className="mt-1 text-xs text-muted-foreground">
                探测列出该 key 能访问的资源（如 OSS bucket、COS bucket、账号/域名）。先验证通过再看结果。
              </p>
            </TabsContent>
          </Tabs>
        </DialogContent>
      </Dialog>
    </div>
  )
}
