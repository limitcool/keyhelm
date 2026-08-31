import { useState } from "react"
import { Cloud, FolderOpen, Boxes, ChevronDown, ChevronRight } from "lucide-react"
import { cn } from "@/lib/utils"
import { CLOUD_PROVIDERS, type Project } from "@/lib/api"
import { ProviderIcon } from "@/components/ProviderIcon"
import { iconByName } from "@/components/ProjectIconPicker"
import { CATEGORIES, projectCategory, projectIcon } from "@/lib/projectMeta"

interface SidebarProps {
  projects: Project[]
  selectedProject: string | null
  selectedService: string | null
  onSelect: (project: string | null, service?: string | null) => void
}

export function Sidebar({
  projects,
  selectedProject,
  selectedService,
  onSelect,
}: SidebarProps) {
  const total = projects.reduce((s, p) => s + p.count, 0)
  const cloudProjects = projects.filter((p) =>
    CLOUD_PROVIDERS.some((c) => c.name === p.project)
  )

  // 折叠状态：分类 id → 是否折叠；默认全展开，折叠由用户操作记忆
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(() => {
    try {
      const raw = localStorage.getItem("kh-sidebar-collapsed")
      return raw ? (JSON.parse(raw) as Record<string, boolean>) : {}
    } catch {
      return {}
    }
  })
  const isCollapsed = (catId: string) =>
    collapsed[catId] === true &&
    // 选中项目所在的分组自动展开，避免折叠后看不到当前项
    !(selectedProject && projectCategory(selectedProject) === catId)
  const toggle = (catId: string) =>
    setCollapsed((c) => {
      const next = { ...c, [catId]: !(c[catId] === true) }
      try {
        localStorage.setItem("kh-sidebar-collapsed", JSON.stringify(next))
      } catch {
        /* ignore */
      }
      return next
    })

  const itemClass = (active: boolean) =>
    cn(
      "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
      active
        ? "bg-accent text-accent-foreground font-medium"
        : "text-muted-foreground hover:bg-muted hover:text-foreground"
    )

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto p-3">
      {/* 全部 */}
      <button
        className={itemClass(selectedProject === null)}
        onClick={() => onSelect(null)}
      >
        <Boxes className="size-4 shrink-0" />
        <span className="truncate">全部</span>
        <span className="ml-auto text-xs tabular-nums text-muted-foreground">
          {total}
        </span>
      </button>

      {/* 云厂商区 */}
      {cloudProjects.length > 0 && (
        <div className="flex flex-col gap-0.5">
          <div className="flex items-center gap-1.5 px-2 pb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <Cloud className="size-3.5" />
            云厂商
          </div>
          {CLOUD_PROVIDERS.map((c) => {
            const p = projects.find((x) => x.project === c.name)
            if (!p) return null
            return (
              <button
                key={c.name}
                className={itemClass(selectedProject === c.name)}
                onClick={() => onSelect(c.name)}
                title={c.supported ? undefined : `${c.label} 尚未接入（后端未实现）`}
              >
                <ProviderIcon provider={c.name} colored className="size-4 shrink-0" />
                <span className="truncate">{c.label}</span>
                <span className="ml-auto flex items-center gap-1">
                  {!c.supported && (
                    <span className="text-[10px] text-muted-foreground/60">未接入</span>
                  )}
                  <span className="text-xs tabular-nums text-muted-foreground">
                    {p.count}
                  </span>
                </span>
              </button>
            )
          })}
        </div>
      )}

      {/* 普通项目（按分类分组） */}
      {(() => {
        const normal = projects.filter((p) => !cloudProjects.includes(p))
        if (normal.length === 0) {
          return (
            <div className="flex flex-col gap-0.5">
              <div className="flex items-center gap-1.5 px-2 pb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                <FolderOpen className="size-3.5" />
                项目
              </div>
              <div className="px-2 py-1 text-xs text-muted-foreground">暂无项目</div>
            </div>
          )
        }
        // 按分类分组，保持分类顺序
        const groups = CATEGORIES.map((c) => ({
          cat: c,
          items: normal.filter((p) => projectCategory(p.project) === c.id),
        })).filter((g) => g.items.length > 0)
        const other = normal.filter((p) => projectCategory(p.project) === "other")
        if (other.length > 0) {
          groups.push({
            cat: { id: "other", label: "其他", icon: FolderOpen },
            items: other,
          })
        }
        return (
          <div className="flex flex-col gap-3">
            {groups.map(({ cat, items }) => {
              const CatIcon = cat.icon
              const closed = isCollapsed(cat.id)
              return (
                <div key={cat.id} className="flex flex-col gap-0.5">
                  <button
                    className="flex w-full items-center gap-1.5 rounded px-2 py-0.5 text-left text-xs font-medium uppercase tracking-wide text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                    onClick={() => toggle(cat.id)}
                    title={closed ? `展开 ${cat.label}` : `折叠 ${cat.label}`}
                  >
                    {closed ? (
                      <ChevronRight className="size-3.5 shrink-0" />
                    ) : (
                      <ChevronDown className="size-3.5 shrink-0" />
                    )}
                    <CatIcon className="size-3.5 shrink-0" />
                    <span>{cat.label}</span>
                    <span className="ml-auto tabular-nums">{items.length}</span>
                  </button>
                  {!closed &&
                    items.map((p) => {
                      const CustomIcon = iconByName(p.icon)
                      const Icon = CustomIcon ?? projectIcon(p.project)
                      return (
                        <button
                          key={p.project}
                          className={itemClass(selectedProject === p.project)}
                          onClick={() => onSelect(p.project)}
                          title={p.project}
                        >
                          <Icon className="size-4 shrink-0 text-muted-foreground" />
                          <span className="truncate">{p.project}</span>
                          <span className="ml-auto text-xs tabular-nums text-muted-foreground">
                            {p.count}
                          </span>
                        </button>
                      )
                    })}
                </div>
              )
            })}
          </div>
        )
      })()}

      {/* 选中项目下的服务（子过滤） */}
      {selectedProject && (
        (() => {
          const p = projects.find((x) => x.project === selectedProject)
          const services = p?.services?.filter(Boolean) ?? []
          if (services.length <= 1) return null
          return (
            <div className="flex flex-col gap-0.5 border-t pt-2">
              <div className="px-2 pb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                服务
              </div>
              {services.map((svc) => (
                <button
                  key={svc}
                  className={itemClass(selectedService === svc)}
                  onClick={() => onSelect(selectedProject, svc)}
                >
                  <span className="truncate pl-5">· {svc}</span>
                </button>
              ))}
            </div>
          )
        })()
      )}
    </div>
  )
}
