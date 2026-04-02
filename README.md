# 🤖 Skills探索 — 本地 AI 技能系统 & Web 助手

这是一个强大的本地 AI 技能框架。除了传统的终端交互，现在还支持一个**全功能的 Web 界面**（基于 Vue 3 + FastAPI），让您能够直观地管理模型、技能和对话。

## ✨ 主要功能

- **现代 Web UI**：玻璃拟态（Glassmorphism）设计，支持响应式布局和深色模式。
- **大模型编排**：支持 SiliconFlow 平台，并允许在 UI 中**自定义 API 网址和 Token**，适配任何 OpenAI 兼容接口。
- **技能管理**：
  - 自动扫描 `skills/` 目录下的 Python 脚本并注册为工具。
  - 在网页端一键开关（Enable/Disable）特定技能。
- **本地工具集成**：天气查询、系统监控、时间感知、文件编辑（安全沙箱）、终端操作、Python 代码写入等。
- **长期记忆**：自动记录用户的偏好和背景信息，实现跨会话记忆。

## 📁 目录结构

```text
Skills探索/
├── webapp/             # ✨ 新增：Web 应用全栈代码
│   ├── backend/        # FastAPI 后端服务
│   └── frontend/       # Vue 3 现代前端界面
├── siliconflow/        # 核心逻辑与 CLI 编排器
│   ├── scripts/chat.py # 终端对话引擎
│   └── .env            # 基础 API 配置
├── skills/             # 本地功能插件库 (Weather, Clock, System...)
└── test/               # terminal 技能的专用沙箱
```

## 🚀 快速入门

### 1. 运行 Web 版本 (推荐)

**后端启动：**
```bash
# 进入后端工作目录并激活虚拟环境
source webapp/backend/venv/bin/activate
# 启动 API 服务
python3 webapp/backend/main.py
```
*API 服务默认运行在: `http://localhost:8000`*

**前端启动：**
```bash
# 进入前端目录并安装依赖 (仅首次)
cd webapp/frontend
npm install
# 开启开发服务器
npm run dev
```
*在浏览器打开输出的地址（通常是 `http://localhost:5173`）即可开始对话。*

### 2. 运行 CLI 版本 (备选)

若您偏好终端，仍可使用原逻辑：
```bash
python3 siliconflow/scripts/chat.py
```

## 🛠️ 已集成技能

| 技能图标 | 技能名称 | 描述说明 | 权限限制 |
| :--- | :--- | :--- | :--- |
| ⛅ | **天气查询** | 获取全球城市实时天气、温度与状态 | 无 |
| 🕒 | **时钟日历** | 获取当前精确的北京时间、日期及星期 | 无 |
| 📊 | **系统监控** | 实时查看 CPU、内存占用及系统运行时间 | 无 |
| 📝 | **文件编辑** | 列出、读取、写入、追加或局部替换文本 | 限于 `Skills探索/` 目录下 |
| 💻 | **终端命令** | 执行 ls, cat, mkdir, grep 等白名单命令 | 限于 `test/` 沙箱内 |
| 🐍 | **Python 写入** | 安全地编写并保存 Python 脚本进行本地处理 | 已集成安全校验 |
| 🧠 | **记忆管理** | 持久化保存或删除关键信息，实现长期记忆 | 自动持久化 |

---
*Powered by Antigravity AI Orchestrator*
