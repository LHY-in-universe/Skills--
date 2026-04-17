# Rust 后端架构说明

## 目标

用 Rust 替代当前 Python FastAPI 主后端，并把现有“单大编排器”拆成清晰的分层架构。

## 分层

### `api`

职责：

- 对外暴露 HTTP / SSE / WebSocket 接口
- 只做协议层的参数解析、错误映射、响应包装

禁止：

- 直接做 provider 调用
- 直接拼接运行态状态机
- 直接读写业务文件

### `app`

职责：

- 组织业务 use case
- 调度各类服务
- 维护运行流程，而不是协议细节

例子：

- 聊天主流程
- resume/abort
- 配置快照读取
- 会话切换

### `domain`

职责：

- 定义核心模型
- 定义状态机输入输出
- 定义稳定枚举和错误码

这一层必须尽可能纯，不依赖具体 HTTP 或文件系统。

### `infra`

职责：

- 配置加载
- SQLite
- provider HTTP 调用
- 外部 skill 调用
- 语音兼容层

## 当前第一阶段完成情况

- 已完成服务骨架
- 已完成配置快照模型
- 已完成基础只读 API
- 已完成 doctor 静态预检雏形
- 已完成 SQLite 会话存储雏形
- 已完成旧 `conversations.json` 自动导入
- 已完成 conversations/history 基础接口

## 后续迁移顺序

1. 配置与 SQLite
2. 会话执行器与对话主链路
3. skills / tools
4. memory / compression
5. voice / diagnostics / observability
