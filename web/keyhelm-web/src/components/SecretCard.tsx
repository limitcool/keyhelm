import { useState } from "react"
import {
  Copy,
  Eye,
  Pencil,
  Trash2,
  KeyRound,
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
import type { Secret } from "@/lib/api"

interface SecretCardProps {
  secret: Secret
  onCopy: (value: string, keyName: string) => void
  onEdit: (secret: Secret) => void
  onDelete: (secret: Secret) => void
}

// 长值在卡片内省略号截断，点击眼睛查看完整
function truncateMiddle(s: string, maxLen = 48): string {
  if (s.length <= maxLen) return s
  const keep = Math.floor((maxLen - 1) / 2)
  return `${s.slice(0, keep)}…${s.slice(-keep)}`
}

export function SecretCard({
  secret,
  onCopy,
  onEdit,
  onDelete,
}: SecretCardProps) {
  const [revealed, setRevealed] = useState(false)
  const value = secret.value ?? ""
  const display = revealed ? value : truncateMiddle(value)

  return (
    <Card className="group relative flex h-full flex-col transition-all duration-150 hover:-translate-y-0.5 hover:border-primary/40 hover:shadow-md">
      <CardHeader className="gap-1 pb-2">
        <div className="flex items-start justify-between gap-2">
          <CardTitle className="flex min-w-0 items-center gap-1.5 text-sm font-semibold">
            <KeyRound className="size-4 shrink-0 text-muted-foreground" />
            <span className="truncate font-mono">{secret.key_name}</span>
          </CardTitle>
          <div className="flex shrink-0 gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="复制"
              onClick={() => onCopy(value, secret.key_name)}
            >
              <Copy />
            </Button>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="查看"
              onClick={() => setRevealed((v) => !v)}
            >
              <Eye />
            </Button>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="编辑"
              onClick={() => onEdit(secret)}
            >
              <Pencil />
            </Button>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="删除"
              onClick={() => onDelete(secret)}
            >
              <Trash2 className="text-destructive" />
            </Button>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge variant="outline" className="font-normal">
            {secret.project}
          </Badge>
          {secret.service ? (
            <Badge variant="secondary" className="font-normal">
              {secret.service}
            </Badge>
          ) : null}
        </div>
      </CardHeader>

      <CardContent className="flex-1 pb-2">
        <code
          className={cn(
            "block w-full truncate rounded-md bg-muted px-2.5 py-1.5 font-mono text-[0.8rem] leading-relaxed text-foreground",
            revealed && "whitespace-normal break-all"
          )}
          title={value}
        >
          {value ? display : "（空值）"}
        </code>
      </CardContent>

      <CardFooter className="flex items-center justify-between gap-2 pb-3">
        <div className="flex flex-wrap gap-1">
          {(secret.tags || []).slice(0, 3).map((t) => (
            <Badge key={t} variant="default" className="bg-muted text-muted-foreground font-normal hover:bg-muted">
              {t}
            </Badge>
          ))}
        </div>
        <span className="shrink-0 text-xs text-muted-foreground">
          {(secret.updated_at || "").slice(0, 16).replace("T", " ")}
        </span>
      </CardFooter>
    </Card>
  )
}
