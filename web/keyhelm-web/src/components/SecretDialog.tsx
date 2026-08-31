import { useEffect, useState } from "react"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { CLOUD_PROVIDERS, setProjectIcon, type Project, type Secret, type SecretPayload } from "@/lib/api"
import { ProjectIconPicker } from "@/components/ProjectIconPicker"

interface SecretDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  secret: Secret | null // null = 新增
  projects: Project[]
  defaultProject?: string | null
  onSave: (payload: SecretPayload, id?: string) => Promise<void>
}

export function SecretDialog({
  open,
  onOpenChange,
  secret,
  projects,
  defaultProject,
  onSave,
}: SecretDialogProps) {
  const [project, setProject] = useState("")
  const [service, setService] = useState("")
  const [keyName, setKeyName] = useState("")
  const [value, setValue] = useState("")
  const [description, setDescription] = useState("")
  const [tags, setTags] = useState("")
  const [icon, setIcon] = useState("")
  const [error, setError] = useState("")
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (!open) return
    setError("")
    setProject(secret?.project ?? defaultProject ?? "")
    setService(secret?.service ?? "")
    setKeyName(secret?.key_name ?? "")
    setValue("") // 编辑时不预填明文
    setDescription(secret?.description ?? "")
    setTags((secret?.tags ?? []).join(", "))
    setIcon(secret ? "" : (projects.find((p) => p.project === project)?.icon ?? ""))
  }, [open, secret, defaultProject])

  // 输入的项目名是否已存在（新建项目 → 显示图标选择器）
  const trimmedProject = project.trim()
  const isNewProject =
    !!trimmedProject &&
    !projects.some((p) => p.project === trimmedProject) &&
    !CLOUD_PROVIDERS.some((c) => c.name === trimmedProject)

  // 已有项目 + 云厂商作为 datalist 建议（允许自由输入新项目）
  const projectOptions = [
    ...CLOUD_PROVIDERS.map((c) => ({ value: c.name, label: c.label })),
    ...projects
      .filter((p) => !CLOUD_PROVIDERS.some((c) => c.name === p.project))
      .map((p) => ({ value: p.project, label: p.project })),
  ].filter((o, i, arr) => arr.findIndex((x) => x.value === o.value) === i)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    if (!project.trim() || !keyName.trim()) {
      setError("项目和键名必填")
      return
    }
    setSaving(true)
    setError("")
    try {
      const payload: SecretPayload = {
        project: project.trim(),
        service: service.trim() || undefined,
        key_name: keyName.trim(),
        value: value.trim() ? value.trim() : undefined,
        description: description.trim() || undefined,
        tags: tags.split(",").map((s) => s.trim()).filter(Boolean),
      }
      await onSave(payload, secret?.id)
      // 新建项目时若选了图标，持久化到后端（失败不影响保存）
      if (!secret && isNewProject && icon) {
        try {
          await setProjectIcon(trimmedProject, icon)
        } catch (e) {
          console.warn("保存图标失败:", (e as Error).message)
        }
      }
      onOpenChange(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败")
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>{secret ? "编辑密钥" : "新增密钥"}</DialogTitle>
            <DialogDescription>
              {secret
                ? `编辑 ${secret.project}/${secret.key_name}。值为空表示不修改。`
                : "创建一个新的密钥。项目必填，键名用大写 env 风格。"}
            </DialogDescription>
          </DialogHeader>
          <div className="grid grid-cols-2 gap-3 py-4">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="s-project">项目 *</Label>
              <Input
                id="s-project"
                value={project}
                onChange={(e) => setProject(e.target.value)}
                placeholder="选择或输入新项目名"
                list="s-project-suggestions"
                autoComplete="off"
              />
              <datalist id="s-project-suggestions">
                {projectOptions.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label !== o.value ? o.label : undefined}
                  </option>
                ))}
              </datalist>
            </div>
            {isNewProject && (
              <div className="col-span-2">
                <ProjectIconPicker value={icon} onChange={setIcon} />
              </div>
            )}
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="s-service">服务</Label>
              <Input
                id="s-service"
                value={service}
                onChange={(e) => setService(e.target.value)}
                placeholder="如 docker-compose"
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="s-key">键名 *</Label>
              <Input
                id="s-key"
                value={keyName}
                onChange={(e) => setKeyName(e.target.value)}
                placeholder="如 ANTHROPIC_API_KEY"
                className="font-mono"
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="s-value">值{secret ? "（留空不变）" : ""}</Label>
              <Input
                id="s-value"
                value={value}
                onChange={(e) => setValue(e.target.value)}
                placeholder="密钥明文"
                type={secret ? "password" : "text"}
                className="font-mono"
                autoComplete="off"
              />
            </div>
            <div className="col-span-2 flex flex-col gap-1.5">
              <Label htmlFor="s-desc">描述</Label>
              <Input
                id="s-desc"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="用途说明"
              />
            </div>
            <div className="col-span-2 flex flex-col gap-1.5">
              <Label htmlFor="s-tags">标签（逗号分隔）</Label>
              <Input
                id="s-tags"
                value={tags}
                onChange={(e) => setTags(e.target.value)}
                placeholder="ai,cloud"
              />
            </div>
          </div>
          {error && <p className="mb-3 text-sm font-medium text-destructive">{error}</p>}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              取消
            </Button>
            <Button type="submit" disabled={saving}>
              {saving ? "保存中…" : "保存"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
