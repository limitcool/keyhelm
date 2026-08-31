// 项目分类 + 图标映射：让侧栏/卡片对项目有可读的归属和图标。
// 云厂商用 ProviderIcon（品牌 logo）；非云厂商按关键词匹配 lucide 图标。

import {
  Bot,
  Brain,
  Sparkles,
  Server,
  Container,
  Database,
  Cpu,
  KeyRound,
  Shield,
  Fingerprint,
  AppWindow,
  Globe,
  MessageSquare,
  Activity,
  Gauge,
  Terminal,
  Monitor,
  Mail,
  Lock,
  Zap,
  Puzzle,
  Boxes,
  FolderOpen,
  Home,
  type LucideIcon,
} from "lucide-react"

/** 项目分类 */
export type ProjectCategory =
  | "ai"
  | "infra"
  | "auth"
  | "app"
  | "monitor"
  | "other"

export interface CategoryMeta {
  id: ProjectCategory
  label: string
  icon: LucideIcon
}

export const CATEGORIES: CategoryMeta[] = [
  { id: "ai", label: "AI / LLM", icon: Bot },
  { id: "infra", label: "基础设施", icon: Server },
  { id: "auth", label: "认证 / 身份", icon: KeyRound },
  { id: "app", label: "应用服务", icon: AppWindow },
  { id: "monitor", label: "监控 / 运维", icon: Activity },
]

// 项目名 → 分类（精确映射优先，含云厂商）
const PROJECT_CATEGORY: Record<string, ProjectCategory> = {
  // 云厂商（品牌 icon，归 ai 展示但侧栏单独分区）
  aliyun: "ai",
  tencent: "ai",
  cloudflare: "infra",
  "google-cloud": "ai",
  aws: "ai",
  azure: "ai",
  digitalocean: "ai",
  // AI / LLM 网关与 AI 服务
  grok2api: "ai",
  "grok2api-egress-quality-guard": "ai",
  "grok-register": "ai",
  "grok-register-lite": "ai",
  groklite8: "ai",
  grokiq: "ai",
  "grokiq-backend": "ai",
  newapi: "ai",
  chatgpt2api: "ai",
  composio: "ai",
  openai: "ai",
  "any-auto-register": "ai",
  // 基础设施
  caddy: "infra",
  minio: "infra",
  postgres: "infra",
  "postgres-main": "infra",
  redis: "infra",
  "shared-redis": "infra",
  forgepanel: "infra",
  "forgepanel-control-plane": "infra",
  komodo: "infra",
  "komodo-periphery": "infra",
  portainer: "infra",
  docker: "infra",
  // 认证 / 身份
  bitwarden: "auth",
  tinyauth: "auth",
  "pocket-id": "auth",
  rustdesk: "auth",
  "rustdesk-server": "auth",
  gatewarden: "auth",
  authelia: "auth",
  authentik: "auth",
  // 应用服务
  sub2api: "app",
  "termi-blog": "app",
  "termi-blog-backend": "app",
  "termi-blog-backend-worker": "app",
  "termi-blog-frontend": "app",
  accounts: "app",
  "accounts-api": "app",
  resin: "app",
  baihu: "app",
  "wa-app": "app",
  "hermes-agent": "app",
  tuwunel: "app",
  "tuwunel-homeserver-1": "app",
  napcat: "app",
  outlook: "app",
  "outlook-email": "app",
  homeassistant: "app",
  homepage: "app",
  matrix: "app",
  mautrix: "app",
  "mautrix-telegram": "app",
  ntfy: "app",
  speedtest: "app",
  gitea: "app",
  "code-server": "app",
  affine: "app",
  bytestash: "app",
  "nas-tool": "app",
  talebook: "app",
  metatube: "app",
  hoppscotch: "app",
  jackett: "app",
  flaresolverr: "app",
  music: "app",
  "music-assistant": "app",
  n8n: "app",
  homebox: "app",
  // 监控 / 运维
  grafana: "monitor",
  prometheus: "monitor",
  loki: "monitor",
  gatus: "monitor",
  watchtower: "monitor",
  pgadmin: "monitor",
  pghero: "monitor",
  "node-exporter": "monitor",
  "postgres-exporter": "monitor",
  "redis-exporter": "monitor",
  goaccess: "monitor",
  "label": "monitor",
}

// 关键词 → 分类（兜底，用于新项目自动归类）
const CATEGORY_KEYWORDS: { category: ProjectCategory; words: string[] }[] = [
  { category: "ai", words: ["grok", "chatgpt", "openai", "llm", "gpt", "deepseek", "claude", "ai-", "composio", "register", "sub2api", "newapi", "baihu"] },
  { category: "infra", words: ["caddy", "minio", "postgres", "redis", "nginx", "traefik", "dns", "proxy", "forge", "komodo", "docker", "portainer", "s3", "db", "sql", "mq", "kafka", "registry", "vpn", "wireguard", "tailscale", "clash", "mihomo"] },
  { category: "auth", words: ["auth", "bitwarden", "vault", "keycloak", "oauth", "sso", "ldap", "passkey", "pocket", "tinyauth", "rustdesk", "gatewarden"] },
  { category: "monitor", words: ["grafana", "prometheus", "loki", "gatus", "watchtower", "uptime", "monitor", "alert", "pgadmin", "pghero", "exporter", "goaccess"] },
  { category: "app", words: ["mail", "blog", "home", "matrix", "ntfy", "chat", "wiki", "cms", "photo", "music", "media", "download", "torrent", "sync", "note", "book", "app", "service", "api"] },
]

/** 项目 → 图标（返回 lucide 图标组件；云厂商品牌图标用 ProviderIcon 渲染） */
export function projectCategory(name: string): ProjectCategory {
  if (PROJECT_CATEGORY[name]) return PROJECT_CATEGORY[name]
  const lower = name.toLowerCase()
  for (const { category, words } of CATEGORY_KEYWORDS) {
    if (words.some((w) => lower.includes(w))) return category
  }
  return "other"
}

// 项目名 → 具体 lucide 图标（精确映射，未命中则用分类默认）
const PROJECT_ICON: Record<string, LucideIcon> = {
  // AI
  grok2api: Brain,
  "grok2api-egress-quality-guard": Shield,
  grok: Brain,
  "grok-register": Bot,
  "grok-register-lite": Bot,
  groklite8: Bot,
  grokiq: Cpu,
  "grokiq-backend": Cpu,
  newapi: Zap,
  chatgpt2api: MessageSquare,
  composio: Puzzle,
  openai: Sparkles,
  deepseek: Sparkles,
  // 基础设施
  caddy: Globe,
  minio: Database,
  "postgres-main": Database,
  postgres: Database,
  redis: Database,
  "shared-redis": Database,
  forgepanel: Server,
  "forgepanel-control-plane": Server,
  komodo: Container,
  "komodo-periphery": Container,
  portainer: Container,
  docker: Container,
  // 认证
  bitwarden: Lock,
  tinyauth: Shield,
  "pocket-id": Fingerprint,
  rustdesk: Monitor,
  "rustdesk-server": Monitor,
  gatewarden: Shield,
  authelia: Lock,
  // 应用
  sub2api: Zap,
  "termi-blog": Globe,
  "termi-blog-backend": Globe,
  "termi-blog-backend-worker": Globe,
  "termi-blog-frontend": Globe,
  accounts: AppWindow,
  "accounts-api": AppWindow,
  resin: Boxes,
  baihu: MessageSquare,
  "wa-app": MessageSquare,
  "hermes-agent": Bot,
  "tuwunel-homeserver-1": Home,
  napcat: MessageSquare,
  "outlook-email": Mail,
  outlook: Mail,
  homeassistant: Home,
  homepage: AppWindow,
  matrix: MessageSquare,
  mautrix: MessageSquare,
  "mautrix-telegram": MessageSquare,
  ntfy: MessageSquare,
  "code-server": Terminal,
  // 监控
  grafana: Gauge,
  prometheus: Activity,
  loki: Activity,
  gatus: Activity,
  watchtower: Shield,
  pgadmin: Database,
  pghero: Database,
  "node-exporter": Activity,
  "postgres-exporter": Activity,
  "redis-exporter": Activity,
  goaccess: Activity,
}

const CATEGORY_DEFAULT_ICON: Record<ProjectCategory, LucideIcon> = {
  ai: Bot,
  infra: Server,
  auth: KeyRound,
  app: AppWindow,
  monitor: Activity,
  other: FolderOpen,
}

/** 项目图标：精确映射命中用精确；否则用分类默认 */
export function projectIcon(name: string): LucideIcon {
  return PROJECT_ICON[name] ?? CATEGORY_DEFAULT_ICON[projectCategory(name)]
}
