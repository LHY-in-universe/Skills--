# Skills探索 — 本地 AI Agent 系统

本地运行的智能 Agent 系统。AI 可调用本地技能（天气、文件操作、终端命令、Python 编写等），所有高危操作需经前端人工审批后才能执行。

## 架构

```
Skills探索/
├── webapp/
│   ├── backend/            # FastAPI 编排器（工具分发、权限管理）
│   │   ├── main.py         # API 服务器
│   │   └── orchestrator.py # AI 对话 + 工具执行核心
│   └── frontend/           # Vue 3 + Vite 前端
│       └── src/
│           ├── App.vue             # 主状态管理、权限弹窗
│           ├── components/
│           │   ├── Sidebar.vue     # 模型选择、技能开关、工作目录设置
│           │   ├── ChatContainer.vue
│           │   └── MessageInput.vue
│           └── index.css
├── skills/                 # 本地技能插件（每个独立的 Python 脚本）
│   ├── terminal/           # 沙箱终端执行
│   ├── file_editor/        # 文件读写
│   ├── python_writer/      # Python 代码生成写入
│   ├── weather/
│   ├── clock/
│   ├── system_monitor/
│   └── monte_carlo/
├── siliconflow/
│   ├── skill_registry.json # 技能注册表（AI 工具接口定义）
│   ├── memory.json         # 长期记忆
│   └── .env                # API 密钥配置（不要提交到 Git）
└── webapp/desktop/         # Electron 桌面小精灵（alwaysOnTop 浮窗）
```

## 快速启动

**后端：**
```bash
cd webapp/backend
source venv/bin/activate
python3 main.py
# 运行在 http://localhost:8000
```x x

**前端：**
```bash
cd webapp/frontend
npm run dev
# 浏览器访问 http://localhost:5173
```

**桌面小精灵（可选）：**
```bash
cd webapp/desktop
npm install  # 首次需要，下载 Electron
npm run start:dev
# 悬浮窗口出现在屏幕右侧，可拖拽到任意位置
```

## 配置

编辑 `siliconflow/.env`：
```
SILICONFLOW_API_KEY=your_api_key_here
SILICONFLOW_API_URL=https://api.siliconflow.cn/v1/chat/completions
```

**重要：** 将 `.env` 加入 `.gitignore`，不要将 API 密钥提交到 Git。

## 已集成技能

| 技能 | 工具名 | 权限要求 |
|------|--------|---------|
| 天气查询 | `get_weather` | 无 |
| 当前时间 | `get_current_time` | 无 |
| 系统监控 | `get_system_info` | 无 |
| 文件操作 | `file_editor` | 读取无需，写入需审批 |
| 终端命令 | `run_terminal` | 每次需审批（删除操作禁用自动授权） |
| Python 写入 | `write_python` | 需审批 |
| 长期记忆 | `memory_save` / `memory_forget` | 无 |
| 蒙特卡洛积分 | `monte_carlo_integration` | 无 |

## 添加新技能

1. 在 `skills/<技能名>/scripts/` 下创建 Python 脚本
2. 在 `siliconflow/skill_registry.json` 中注册（工具名、描述、参数定义）
3. 重启后端，侧边栏自动出现该技能的开关

## 终端工作目录

侧边栏"终端工作目录"区块可设置 `run_terminal` 的执行目录。该设置优先级高于 AI 自身的判断，AI 无法覆盖此路径。不设置时使用默认沙箱 `test/`。

---

## 安全机制

### 人类在环（Human-in-the-Loop）

所有写操作在执行前都会在前端弹出审批弹窗，用户可选择：
- **同意执行**：本次授权
- **一直同意**：缓存本类操作的授权（`rm` 类删除命令除外，每次必须手动审批）
- **拒绝**：取消本次执行

### 终端沙箱（run_terminal）

- **命令白名单**：只允许 `ls/cat/head/tail/grep/find/mkdir/touch/cp/mv/rm/sort/python3` 等
- **python3 限制**：禁止 `-c`、`-m` 等任意代码执行参数，只允许运行 `.py` 文件
- **危险字符拦截**：`$(` `` ` `` `&&` `||` `;` `>` `>>` `<` `|` `../` 全部拦截
- **路径限制**：绝对路径必须在工作目录内（使用 `Path.relative_to()` 严格校验，防止符号链接绕过）
- **删除强制审批**：`rm`/`rmdir` 使用正则匹配，即使开启"一直同意"也每次弹窗

### 文件操作沙箱（file_editor / write_python）

- 所有路径使用 `Path.relative_to()` 校验，防止 `../` 路径穿越和符号链接绕过
- `write_python` 对写入代码做语法检查、禁用危险 API（`eval`/`exec`/`os.system`/`subprocess` 等）、强制结构规范

### 前端安全

- Markdown 渲染禁用原始 HTML（`html: false`），防止 AI 返回内容中嵌入 XSS 脚本
- CORS 限制为 `localhost:5173/5174`，禁止其他来源跨域调用

### 已知限制（本地开发工具）

- API 端点无认证，仅适合本机使用，不要将后端暴露到公网
- 终端工具的 AppleScript 可视镜像功能使用字符串转义，建议不要在命令输出中包含特殊字符组合

---

## 桌面小精灵（Electron）

Electron 将现有 Vue 前端包装为悬浮桌面窗口：

- **始终置顶**：悬浮于所有应用窗口之上
- **可拖拽**：顶部拖拽条，可移动到屏幕任意位置
- **折叠模式**：点击"—"缩为 60×60 浮球，点击浮球展开
- **复用后端**：与浏览器版共用同一个 FastAPI 后端

启动需同时运行后端 + 前端 dev server + `npm run start:dev`。
