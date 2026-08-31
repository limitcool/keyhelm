import { useState } from "react"
import {
  Copy,
  Eye,
  Pencil,
  Trash2,
  Building2,
  User,
  RefreshCw,
  Loader2,
  Cloud,
} from "lucide-react"

import {
  Card,
  CardContent,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { CLOUD_PROVIDERS, cloudProbe, type Secret } from "@/lib/api"
import { ProviderIcon } from "@/components/ProviderIcon"
import { iconByName } from "@/components/ProjectIconPicker"
import { projectIcon } from "@/lib/projectMeta"
import { toast } from "sonner"

interface CredentialCardProps {
  project: string
  /** 项目自定义 lucide 图标名（可选，空则用 projectMeta 兜底） */
  projectIconName?: string
  service: string | null
  identity: string
  probeData?: Record<string, unknown>
  secrets: Secret[]
  onCopy: (value: string, keyName: string) => void
  onEdit: (secret: Secret) => void
  onDelete: (secret: Secret) => void
}

// 探针结果的分组展示名（对应后端 providers.rs 返回的字段）
const PROBE_LABELS: Record<string, string> = {
  buckets: "存储桶",
  policies: "权限策略",
  zones: "域名",
  accounts: "账号",
  projects: "项目",
  regions: "地域",
  errors: "错误",
}

/** 把非机密探针结果渲染成徽章列表（存储桶/权限策略/域名等数组 + 地域等标量） */
function ProbeBadges({ data }: { data: Record<string, unknown> }) {
  const arrayEntries = Object.entries(data).filter(
    ([k, v]) => k !== "errors" && Array.isArray(v) && (v as unknown[]).length > 0
  )
  const scalarEntries = Object.entries(data).filter(
    ([k, v]) =>
      k !== "errors" &&
      k !== "probe" &&
      k !== "provider" &&
      v != null &&
      typeof v !== "object"
  )
  if (arrayEntries.length === 0 && scalarEntries.length === 0) return null
  return (
    <div className="flex flex-col gap-1 border-t border-border/60 pt-2">
      {arrayEntries.map(([k, v]) => (
        <div key={k} className="flex flex-wrap items-center gap-1">
          <span className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
            {PROBE_LABELS[k] ?? k}
          </span>
          {(v as unknown[]).slice(0, 6).map((item, i) => {
            const o =
              typeof item === "object" && item !== null
                ? (item as Record<string, unknown>)
                : null
            const label = o ? (o.name ?? o.id ?? o.accountId) : item
            const sub = o ? (o.type ? String(o.type) : null) : null
            return (
              <Badge
                key={i}
                variant="outline"
                className="font-mono font-normal"
              >
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
      ))}
      {scalarEntries.map(([k, v]: [string, unknown]) => (
        <div key={k} className="flex flex-wrap items-center gap-1">
          <span className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
            {PROBE_LABELS[k] ?? k}
          </span>
          <Badge variant="outline" className="font-mono font-normal">
            {String(v)}
          </Badge>
        </div>
      ))}
    </div>
  )
}

// 长值在卡片内省略号截断，点击眼睛查看完整
function truncateMiddle(s: string, maxLen = 48): string {
  if (s.length <= maxLen) return s
  const keep = Math.floor((maxLen - 1) / 2)
  return `${s.slice(0, keep)}…${s.slice(-keep)}`
}

/**
 * 凭据组卡片：同一个 (project, service) 下的关联密钥（如 ID + KEY）合并为一张卡。
 * 头部直接显示该账号的身份（云厂商探测后写回，如 RAM 用户 / 腾讯 AccountId / GCP client_email），
 * 不需要额外点「验证」按钮。
 */
export function CredentialCard({
  project,
  projectIconName,
  service,
  identity,
  probeData,
  secrets,
  onCopy,
  onEdit,
  onDelete,
}: CredentialCardProps) {
  const [revealed, setRevealed] = useState<Record<string, boolean>>({})
  const [probing, setProbing] = useState(false)
  const cloud = CLOUD_PROVIDERS.find((c) => c.name === project)

  // 卡片内直接探测：刷新该账号可访问的资源/权限（写回 DB，其它卡片共享）
  async function handleProbe() {
    setProbing(true)
    try {
      await cloudProbe(project, project)
      toast.success(`${cloud?.label ?? project} 探测完成`)
    } catch (e) {
      toast.error(`探测失败: ${(e as Error).message}`)
    } finally {
      setProbing(false)
    }
  }

  const hasProbe = probeData && Object.keys(probeData).length > 0

  return (
    <Card className="group relative flex h-full flex-col transition-all duration-150 hover:-translate-y-0.5 hover:border-primary/40 hover:shadow-md">
      <CardHeader className="gap-1.5 pb-2">
        <div className="flex items-center gap-1.5">
          {cloud ? (
            <ProviderIcon provider={cloud.name} colored className="size-4 shrink-0" />
          ) : (() => {
            const CustomIcon = iconByName(projectIconName)
            const Icon = CustomIcon ?? projectIcon(project)
            return <Icon className="size-4 shrink-0 text-muted-foreground" />
          })()}
          <CardTitle
            className="flex min-w-0 items-center gap-1 text-sm font-semibold"
            title={identity || undefined}
          >
            {identity && (
              <User className="size-3.5 shrink-0 text-muted-foreground" />
            )}
            <span className="truncate">
              {identity || cloud?.label || project}
            </span>
          </CardTitle>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant="outline" className="font-normal">
            {project}
          </Badge>
          {service ? (
            <Badge variant="secondary" className="font-normal">
              {service}
            </Badge>
          ) : null}
          {identity && !cloud && (
            <Badge variant="secondary" className="font-normal">
              <Building2 className="mr-0.5 size-3" />
              {identity}
            </Badge>
          )}
        </div>
      </CardHeader>

      <CardContent className="flex flex-1 flex-col gap-1.5 pb-2">
        {secrets.map((s) => {
          const value = s.value ?? ""
          const isOpen = revealed[s.id]
          const display = isOpen ? value : truncateMiddle(value)
          return (
            <div
              key={s.id}
              className="flex flex-col gap-0.5 rounded-md border border-border/60 px-2 py-1.5"
            >
              <div className="flex items-center justify-between gap-1">
                <span
                  className="truncate font-mono text-[0.7rem] text-muted-foreground"
                  title={s.key_name}
                >
                  {s.key_name}
                </span>
                <div className="flex shrink-0 gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`复制 ${s.key_name}`}
                    onClick={() => onCopy(value, s.key_name)}
                  >
                    <Copy />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`查看 ${s.key_name}`}
                    onClick={() =>
                      setRevealed((r) => ({ ...r, [s.id]: !r[s.id] }))
                    }
                  >
                    <Eye />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`编辑 ${s.key_name}`}
                    onClick={() => onEdit(s)}
                  >
                    <Pencil />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`删除 ${s.key_name}`}
                    onClick={() => onDelete(s)}
                  >
                    <Trash2 className="text-destructive" />
                  </Button>
                </div>
              </div>
              <code
                className={cn(
                  "block w-full truncate rounded bg-muted px-2 py-1 font-mono text-[0.8rem] leading-relaxed text-foreground",
                  isOpen && "whitespace-normal break-all"
                )}
                title={value}
              >
                {value ? display : "（空值）"}
              </code>
            </div>
          )
        })}
      </CardContent>

      {/* 权限 + 资源（探针结果，非机密） */}
      <CardContent className="flex flex-col gap-1 pb-2">
        {probing ? (
          <div className="flex items-center gap-2 rounded-md border border-border/60 px-2 py-1.5 text-xs text-muted-foreground">
            <Loader2 className="size-3 animate-spin" /> 正在探测可访问资源…
          </div>
        ) : hasProbe ? (
          <ProbeBadges data={probeData} />
        ) : (
          <div className="flex items-center justify-between gap-2 rounded-md border border-dashed border-border/60 px-2 py-1.5">
            <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <Cloud className="size-3" /> 尚未探测资源
            </span>
            <Button
              variant="ghost"
              size="xs"
              aria-label="探测资源"
              onClick={handleProbe}
            >
              <RefreshCw className="size-3" />
              探测
            </Button>
          </div>
        )}
      </CardContent>

      <CardFooter className="flex items-center justify-between gap-2 pb-3">
        <div className="flex flex-wrap gap-1">
          {secrets[0]?.tags?.slice(0, 3).map((t) => (
            <Badge
              key={t}
              variant="default"
              className="bg-muted text-muted-foreground font-normal hover:bg-muted"
            >
              {t}
            </Badge>
          ))}
        </div>
        <span className="shrink-0 text-xs text-muted-foreground">
          {(secrets[0]?.updated_at || "").slice(0, 16).replace("T", " ")}
        </span>
      </CardFooter>
    </Card>
  )
}
