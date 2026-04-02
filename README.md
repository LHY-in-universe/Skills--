# 🤖 Skills探索 — 智能本地 AI 代理系统

这是一个强大的本地智能 Agent 架构。除了支持传统的命令行（CLI）执行外，还配备了具备**现代化玻璃拟态 UI**的完整 Web 控制台。系统不仅能与大模型进行流畅交流，还能安全、可控地操作您的本地计算机设备。

## ✨ 核心特性

- **现代 Web UI**：玻璃拟态（Glassmorphism）设计、深/浅色模式自适应、工具执行折叠动画显示，聊天顺滑流畅。
- **大模型动态接驳**：支持任何兼容 OpenAI 格式的 API（如 SiliconFlow、DeepSeek、Qwen 接口），可在侧边栏灵活调整 API 令牌和自定义模型模型。
- **🛠️ 动态技能中心（Skill Registry）**：
  - 支持热插拔技能。通过修改 `siliconflow/skill_registry.json`，Web 侧边栏会自动显示新技能开关，无需修改 Web 代码。
  - 完全由数据驱动，按需启用/禁用功能（防幻觉保护）。
- **🔒 安全前置：人类在环（Human-in-the-loop）**：
  - **强制审批机制**：在执行高危操作（新建/修改文件、执行终端命令等）前，执行流会被强制截断并在前端抛出审批弹窗。
  - **沙箱隔离隔离**：对于后台终端指令，所有包含 `sudo`, `rm -rf`, 超越相对路径 (`../`) 等危险语法的命令均会被拦截器阻拦。
- **🖥️ 真实终端可视监控**：通过 `open_terminal` 工具操作时，直接跨进程触发 AppleScript 打开你的 `Terminal.app` 显示并执行命令，不让 AI 黑盒运行。

## 📁 目录结构

```text
Skills探索/
├── webapp/             # Web 应用全栈代码
│   ├── backend/        # FastAPI 编排器 (拦截器、工具解析)
│   └── frontend/       # Vue 3 现代界面 (消息控制、动态菜单)
├── siliconflow/        # 核心技能源数据库与系统变量
│   ├── skill_registry.json # ✨ 技能总注册表（工具的元信息管理）
│   ├── scripts/chat.py # 终端 CLI 备用引擎
│   └── .env            # 基础 API 配置
├── skills/             # 本地能力插件脚本库
└── test/               # terminal 命令执行的安全沙箱
```

## 🚀 快速入门

### 1. 启动完整的 Web 版本 (推荐核心玩法)

**启动 AI 大脑与服务后端：**
```bash
cd /Users/lhy/Desktop/Skills探索/webapp/backend 
source venv/bin/activate
python3 main.py
```
*服务默认运行在 `http://localhost:8000`*

**启动交互前端 (Vite)：**
```bash
cd /Users/lhy/Desktop/Skills探索/webapp/frontend 
npm run dev
```
*在浏览器打开输出的地址（通常是 `http://localhost:5173`）即可开始对话。*

### 2. 添加你的私人技能
1. 在 `skills/你的新功能/scripts/脚本.py` 中编写逻辑。
2. 在 `siliconflow/skill_registry.json` 中配置大模型对应的接口协议参数。
3. 重启 `python3 main.py`，你的浏览器左侧就会神奇地长出这个能力的开关啦！

## 🛠️ 已原生集成的强大技能

| 技能图标 | 技能名称 | 对应后台工具 | 描述说明 & 保护级别 |
| :--- | :--- | :--- | :--- |
| ⛅ | **天气查询** | `get_weather` | 获取全球城市实时天气。已修复多语言 Unicode |
| 🕒 | **时钟日历** | `get_current_time` | 获取当前的精准北京时间 |
| 📊 | **系统监控** | `get_system_info` | 查看 CPU、内存及系统进程 |
| 📝 | **文件操作** | `file_editor` | 查阅、局部修改文件。写入操作**需要前端人工授权确认** |
| 🛡️ | **沙箱隔离命令** | `run_terminal` | 测试白名单脚本执行。**强制前端人工确认审批，路径设限** |
| 🖥️ | **直连真实终端** | `open_terminal` | 唤起 MacOS 真终端执行，实时透明可见。**需要前端人工确认** |
| 🐍 | **编写Python** | `write_python` | 生成代码进硬盘特定路径。**需要前端人工确认** |
| 🧠 | **记忆网络管理** | `memory_save` | 动态将你偏好存入长记忆系统 |
| 🎲 | **高阶运算测试**| `monte_carlo_integration`| 基于离散蒙特卡洛测试的算法求解功能集成 |

---
*Powered by Deepmind AI Agent System - `Antigravity`*
