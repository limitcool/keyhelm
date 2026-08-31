import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      globals: globals.browser,
    },
    rules: {
      // dialog 打开时用 effect 重置表单是标准模式，React 19 新规则误报率高，降级关闭
      'react-hooks/set-state-in-effect': 'off',
    },
  },
  {
    // shadcn/ui 生成的组件常导出 variants 等非组件符号，属正常模式，关闭 fast-refresh 误报
    files: ['src/components/ui/**/*.tsx'],
    rules: {
      'react-refresh/only-export-components': 'off',
    },
  },
  {
    // 该文件同时导出组件与共享图标常量/工具函数（被 Sidebar/Dashboard 引用），属正常布局
    files: ['src/components/ProjectIconPicker.tsx'],
    rules: {
      'react-refresh/only-export-components': 'off',
    },
  },
])
