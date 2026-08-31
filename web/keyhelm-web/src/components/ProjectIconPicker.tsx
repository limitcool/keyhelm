import {
  Activity,
  AppWindow,
  Bell,
  Book,
  Bot,
  Box,
  Boxes,
  Brain,
  Building2,
  Cable,
  Cloud,
  Code2,
  Container,
  Cpu,
  Database,
  Fingerprint,
  Flag,
  FolderOpen,
  Gauge,
  Globe,
  HardDrive,
  Heart,
  Hexagon,
  Home,
  KeyRound,
  Layers,
  Lock,
  Mail,
  MessageSquare,
  Monitor,
  Network,
  Package,
  Rocket,
  Router,
  Search,
  Server,
  Settings,
  Shield,
  Sparkles,
  Star,
  Store,
  Terminal,
  Truck,
  Tv,
  Users,
  Wifi,
  Wrench,
  Zap,
  type LucideIcon,
} from "lucide-react"
import { Label } from "@/components/ui/label"
import { cn } from "@/lib/utils"

/** 可选的 lucide 图标集（key 为存到后端的图标名） */
export const PROJECT_ICON_CHOICES: { name: string; icon: LucideIcon }[] = [
  { name: "Bot", icon: Bot },
  { name: "Sparkles", icon: Sparkles },
  { name: "Brain", icon: Brain },
  { name: "MessageSquare", icon: MessageSquare },
  { name: "Zap", icon: Zap },
  { name: "Server", icon: Server },
  { name: "Globe", icon: Globe },
  { name: "Database", icon: Database },
  { name: "Container", icon: Container },
  { name: "Cpu", icon: Cpu },
  { name: "Network", icon: Network },
  { name: "Router", icon: Router },
  { name: "HardDrive", icon: HardDrive },
  { name: "Cable", icon: Cable },
  { name: "KeyRound", icon: KeyRound },
  { name: "Lock", icon: Lock },
  { name: "Shield", icon: Shield },
  { name: "Fingerprint", icon: Fingerprint },
  { name: "AppWindow", icon: AppWindow },
  { name: "Terminal", icon: Terminal },
  { name: "Code2", icon: Code2 },
  { name: "Mail", icon: Mail },
  { name: "Box", icon: Box },
  { name: "Boxes", icon: Boxes },
  { name: "FolderOpen", icon: FolderOpen },
  { name: "Home", icon: Home },
  { name: "Store", icon: Store },
  { name: "Truck", icon: Truck },
  { name: "Tv", icon: Tv },
  { name: "Gauge", icon: Gauge },
  { name: "Activity", icon: Activity },
  { name: "Monitor", icon: Monitor },
  { name: "Bell", icon: Bell },
  { name: "Flag", icon: Flag },
  { name: "Heart", icon: Heart },
  { name: "Star", icon: Star },
  { name: "Rocket", icon: Rocket },
  { name: "Search", icon: Search },
  { name: "Wrench", icon: Wrench },
  { name: "Settings", icon: Settings },
  { name: "Book", icon: Book },
  { name: "Layers", icon: Layers },
  { name: "Package", icon: Package },
  { name: "Cloud", icon: Cloud },
  { name: "Users", icon: Users },
  { name: "Building2", icon: Building2 },
  { name: "Hexagon", icon: Hexagon },
  { name: "Wifi", icon: Wifi },
]

/** 按图标名找 lucide 组件（供外部用项目 icon 名渲染） */
export function iconByName(name?: string): LucideIcon | null {
  if (!name) return null
  return PROJECT_ICON_CHOICES.find((c) => c.name === name)?.icon ?? null
}

interface ProjectIconPickerProps {
  value: string
  onChange: (name: string) => void
}

/** 项目图标选择器：网格选图标，支持点击已选项取消 */
export function ProjectIconPicker({ value, onChange }: ProjectIconPickerProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label>项目图标（可选）</Label>
      <div className="grid grid-cols-8 gap-1">
        {PROJECT_ICON_CHOICES.map(({ name, icon: Icon }) => (
          <button
            key={name}
            type="button"
            title={name}
            onClick={() => onChange(value === name ? "" : name)}
            className={cn(
              "flex h-8 w-8 items-center justify-center rounded-md border transition-colors",
              value === name
                ? "border-primary bg-primary/10 text-primary"
                : "border-border/60 text-muted-foreground hover:border-primary/40 hover:text-foreground"
            )}
          >
            <Icon className="size-4" />
          </button>
        ))}
      </div>
      {value && (
        <p className="text-xs text-muted-foreground">
          已选 <span className="font-mono">{value}</span>（再点一次取消）
        </p>
      )}
    </div>
  )
}
