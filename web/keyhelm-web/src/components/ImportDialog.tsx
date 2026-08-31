import { useEffect, useState } from "react"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { importSecrets } from "@/lib/api"
import { toast } from "sonner"

interface ImportDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onImported: () => void
}

// 占位示例（含项目/服务/键名/值），用户可整段替换
const PLACEHOLDER = `[
  {
    "project": "myapp",
    "service": "backend",
    "key_name": "DATABASE_PASSWORD",
    "value": "s3cr3t",
    "description": "数据库密码",
    "tags": ["db"]
  }
]`

export function ImportDialog({ open, onOpenChange, onImported }: ImportDialogProps) {
  const [text, setText] = useState("")
  const [error, setError] = useState("")
  const [importing, setImporting] = useState(false)

  useEffect(() => {
    if (!open) return
    setText("")
    setError("")
  }, [open])

  async function submit() {
    setError("")
    let items: unknown
    try {
      items = JSON.parse(text)
    } catch {
      setError("不是合法的 JSON，请检查格式")
      return
    }
    if (!Array.isArray(items)) {
      setError("需要 JSON 数组")
      return
    }
    if (items.length === 0) {
      setError("数组不能为空")
      return
    }
    setImporting(true)
    try {
      const result = await importSecrets(items as never)
      toast.success(
        `导入完成：新建 ${result.created}，更新 ${result.updated}${result.errors?.length ? `，失败 ${result.errors.length}` : ""}`
      )
      onImported()
      onOpenChange(false)
    } catch (e) {
      setError(e instanceof Error ? e.message : "导入失败")
    } finally {
      setImporting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>批量导入</DialogTitle>
          <DialogDescription>
            JSON 数组，每项包含 project / key_name（必填），service /
            description / tags 可选。
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="import-json">JSON 内容</Label>
          <textarea
            id="import-json"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={PLACEHOLDER}
            spellCheck={false}
            className="h-56 w-full resize-y rounded-md border border-border bg-background px-3 py-2 font-mono text-[0.8rem] outline-none focus-visible:ring-1 focus-visible:ring-ring"
          />
          <p className="text-xs text-muted-foreground">
            值将加密存储，不会在日志中明文显示。
          </p>
        </div>
        {error && <p className="text-sm font-medium text-destructive">{error}</p>}
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button type="button" onClick={submit} disabled={importing}>
            {importing ? "导入中…" : "导入"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
