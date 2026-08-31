import { useEffect, useState } from "react"
import { Dashboard } from "@/components/Dashboard"
import { LoginView } from "@/components/LoginView"
import { getProjects, login } from "@/lib/api"

export function App() {
  const [authed, setAuthed] = useState<boolean | null>(null)

  // 启动时探测登录态（cookie session 是否有效）
  useEffect(() => {
    ;(async () => {
      try {
        await getProjects()
        setAuthed(true)
      } catch {
        setAuthed(false)
      }
    })()
  }, [])

  async function handleLogin(username: string, password: string) {
    await login(username, password)
    setAuthed(true)
  }

  function handleLoggedOut() {
    setAuthed(false)
  }

  if (authed === null) {
    // 首屏加载占位
    return (
      <div className="flex min-h-svh items-center justify-center text-muted-foreground">
        加载中…
      </div>
    )
  }

  return authed ? (
    <Dashboard onLoggedOut={handleLoggedOut} />
  ) : (
    <LoginView onLogin={handleLogin} />
  )
}

export default App
