# Skills探索

本仓库包含一个可视化前端（Vue + Vite）、一个后端编排服务（FastAPI），以及独立的 `siliconflow` 命令行编排模块。

## 项目结构

```text
Skills探索/
├── webapp/
│   ├── frontend/           # Vue 3 + Vite 前端
│   ├── backend/            # FastAPI 后端
│   └── desktop/            # Electron 桌面壳（可选）
├── skills/                 # 技能目录（SKILL.md / skill_manifest.json）
└── siliconflow/            # CLI 编排模块与数据目录
```

## 启动前准备

### 1) Python 环境（后端）

```bash
cd webapp/backend
python3 -m venv venv
source venv/bin/activate
pip install fastapi uvicorn openai numpy requests httpx pyyaml lark-oapi
```

说明：

- 当前仓库没有 `webapp/backend/requirements.txt`，所以上面使用的是按代码导入整理出的最小依赖集合。
- 如果启用语音桥接，还需要额外安装：`sherpa-onnx edge-tts pydub`。

### 2) Node 环境（前

```bash
cd webapp/frontend
npm install
```

### 3) 环境变量

后端会读取 `siliconflow/config/.env`（不是 `siliconflow/.env`）。建议至少配置：

```bash
SILICONFLOW_API_KEY=your_api_key
SILICONFLOW_API_URL=https://api.siliconflow.cn/v1/chat/completions

# 可选：DeepSeek
DEEPSEEK_API_KEY=your_deepseek_key
DEEPSEEK_API_URL=https://api.deepseek.com/v1/chat/completions

# 可选：飞书桥接
LARK_APP_ID=cli_xxxxxxxxxxxx
LARK_APP_SECRET=xxxxxxxxxxxxxxxxxxxxxxxx
```

## 前后端启动方式

### 启动后端（FastAPI）

```bash
cd webapp/backend
source venv/bin/activate
python3 main.py
```

默认监听：`http://127.0.0.1:8000`

### 启动前端（Vite）

```bash
cd webapp/frontend
npm run dev
```

默认地址：`http://127.0.0.1:5173`（或 5174）

## 可选组件

### Electron 桌面壳

```bash
cd webapp/desktop
npm install
npm run start
```

### SiliconFlow CLI 模块

参见 [siliconflow/README.md](./siliconflow/README.md)。

## 常用开发命令

```bash
# 检查技能可用性
cd webapp/backend
source venv/bin/activate
python3 verify_skills.py
```

## 技能系统概览

- Markdown 技能：在 `skills/<name>/SKILL.md` 中定义说明与执行模板。
- Native 技能：在 `skills/<name>/skill_manifest.json` 中定义参数，再由脚本执行。
- 高风险操作（如文件改写、终端命令）会走权限确认流程。
