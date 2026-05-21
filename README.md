# 飞鸟下载器 · FeiNiao Downloader

> 极简的视频下载工具，基于 yt-dlp，支持 1800+ 平台。

[![Build](https://github.com/Yorushika-fan/feiniao-downloader/actions/workflows/build.yml/badge.svg)](https://github.com/Yorushika-fan/feiniao-downloader/actions/workflows/build.yml)
[![Website](https://img.shields.io/badge/官网-feiniao--downloader.pages.dev-CA8A04)](https://feiniao-downloader.pages.dev/)

## ✨ 特性

- **1800+ 平台** — YouTube / Bilibili / TikTok / 抖音 / 小红书 / X / Twitch …
- **三大系统** — macOS / Windows / Linux 原生支持
- **零配置启动** — App 自动下载 yt-dlp 内核
- **多语言界面** — 简体 / 繁體 / English / 日本語
- **代理自动检测** — 识别 Clash / Surge / V2Ray
- **任务流** — 进行中 + 历史合并为单一时间流
- **隐私本地化** — 所有处理本机进行，零遥测

## 📦 下载

访问 [官网](https://feiniao-downloader.pages.dev/) 或 [Releases](https://github.com/Yorushika-fan/feiniao-downloader/releases) 获取最新版本。

| 平台 | 下载 |
|---|---|
| macOS (Apple Silicon / Intel) | `.dmg` |
| Windows | `.msi` |
| Linux | `.AppImage` / `.deb` |

## 🏗️ 技术栈

- **Tauri 2** — 桌面外壳（Rust）
- **React 18 + TypeScript** — 前端
- **Vite 5** — 构建工具
- **Tailwind CSS v3** — 样式
- **Radix UI** — 交互原语
- **Zustand** — 状态管理
- **reqwest + tokio** — Rust 异步 HTTP
- **curl_cffi** — TLS 指纹模拟（通过 yt-dlp 内核）

## 🚀 开发

```bash
# 克隆
git clone https://github.com/Yorushika-fan/feiniao-downloader.git
cd feiniao-downloader

# 安装依赖
pnpm install

# 开发模式（热重载）
pnpm tauri:dev

# 打包当前平台
pnpm tauri:build
```

依赖：
- Node.js ≥ 20
- Rust ≥ 1.77
- pnpm ≥ 9

## 📁 目录结构

```
.
├── src/                # React 前端
│   ├── components/     # UI 组件
│   ├── pages/          # home / settings
│   ├── lib/            # tauri API 封装、工具
│   ├── store/          # Zustand 状态
│   └── styles/         # 全局样式
├── src-tauri/          # Rust 后端
│   ├── src/
│   │   ├── lib.rs      # 入口
│   │   ├── commands.rs # Tauri 命令
│   │   ├── ytdlp.rs    # yt-dlp 封装（含跨平台路径）
│   │   ├── proxy.rs    # 跨平台代理检测
│   │   ├── state.rs    # 应用状态
│   │   └── types.rs    # 数据类型
│   └── tauri.conf.json
├── website/            # 官网（Cloudflare Pages）
└── .github/workflows/  # CI 编译
```

## 🤝 致谢

- [yt-dlp](https://github.com/yt-dlp/yt-dlp) — 核心下载引擎
- [Tauri](https://tauri.app/) — 跨平台桌面框架

## ⚠️ 免责声明

本工具仅用于下载用户合法访问的内容。请遵守当地法律和平台服务条款。

## 📝 许可

MIT
