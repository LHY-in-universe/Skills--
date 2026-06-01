<script setup>
import { computed, inject } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { MessageSquarePlus, MessagesSquare, Bot, Wrench, GitBranch, Gauge, Activity, Terminal, KeyRound, Trash2 } from 'lucide-vue-next'
import { getProviderName } from '../lib/modelMeta'

const route = useRoute()

const models = inject('models')
const currentModel = inject('currentModel')
const apiConfig = inject('apiConfig')
const isLightMode = inject('isLightMode')
const conversations = inject('conversations')
const activeConversationId = inject('activeConversationId')
const liveUsage = inject('liveUsage')
const routingConfig = inject('routingConfig')
const appActions = inject('appActions')

const navItems = [
  { to: '/chat', label: '聊天', icon: Bot },
  { to: '/models', label: '模型', icon: Bot },
  { to: '/skills', label: '技能', icon: Wrench },
  { to: '/routing', label: '路由', icon: GitBranch },
  { to: '/runtime', label: '运行时', icon: Gauge },
  { to: '/diagnostics', label: '诊断', icon: Activity },
  { to: '/terminal', label: '终端', icon: Terminal },
  { to: '/lark', label: 'Lark', icon: KeyRound },
]

const runtimeSummary = computed(() => {
  const effectiveModelId = apiConfig.value?.effective_model_id || ''
  const effectiveProvider = apiConfig.value?.effective_provider || ''
  const matchedModel = (models.value || []).find((item) => item.apiId === effectiveModelId) || null
  const model = matchedModel?.displayName || effectiveModelId || currentModel.value || '-'
  const provider = effectiveProvider || (matchedModel ? getProviderName(matchedModel) : '-')
  const routeEnabled = routingConfig.value?.enabled === true
  return { model, provider, routeEnabled }
})

const sortedConversations = computed(() =>
  [...(conversations.value || [])].sort((a, b) => Number(b.active) - Number(a.active))
)

const isRouteActive = (to) => route.path === to
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header">
      <h1 class="logo">SiliconFlow AI</h1>
      <button class="theme-toggle" @click="appActions.toggleTheme()" :title="isLightMode ? '切换到暗色模式' : '切换到亮色模式'">
        {{ isLightMode ? '🌙' : '☀️' }}
      </button>
    </div>

    <div class="sidebar-summary card-lite">
      <div class="summary-row">
        <span class="muted">路由状态</span>
        <strong>{{ runtimeSummary.routeEnabled ? '已启用' : '未启用' }}</strong>
      </div>
      <div class="summary-row">
        <span class="muted">生效 Provider</span>
        <strong>{{ runtimeSummary.provider }}</strong>
      </div>
      <div class="summary-row">
        <span class="muted">生效模型</span>
        <strong>{{ runtimeSummary.model }}</strong>
      </div>
      <div class="summary-row">
        <span class="muted">本轮 Token</span>
        <strong>{{ (liveUsage?.total || 0).toLocaleString() }}</strong>
      </div>
    </div>

    <div class="section-title">导航</div>
    <nav class="sidebar-nav">
      <RouterLink
        v-for="item in navItems"
        :key="item.to"
        :to="item.to"
        class="nav-link"
        :class="{ active: isRouteActive(item.to) }"
      >
        <component :is="item.icon" :size="16" />
        <span>{{ item.label }}</span>
      </RouterLink>
    </nav>

    <div class="section-title section-split">
      <span>对话</span>
      <button class="icon-btn" @click="appActions.createConversation()" title="新建对话">
        <MessageSquarePlus :size="16" />
      </button>
    </div>

    <div class="conversation-list">
      <button
        v-for="conv in sortedConversations"
        :key="conv.id"
        class="conversation-item"
        :class="{ active: conv.id === activeConversationId }"
        @click="appActions.switchConversation(conv.id)"
      >
        <MessagesSquare :size="15" />
        <span>{{ conv.name }}</span>
      </button>
    </div>

    <div class="sidebar-footer">
      <button class="footer-btn" @click="appActions.clearHistory()">
        <Trash2 :size="15" />
        <span>清空当前对话</span>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar-summary {
  display: grid;
  gap: 10px;
  margin-bottom: 18px;
}

.card-lite {
  background: var(--input-bg);
  border: 1px solid var(--border-color);
  border-radius: 14px;
  padding: 14px;
}

.summary-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  font-size: var(--font-xs);
}

.summary-row strong {
  max-width: 160px;
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.muted {
  color: var(--text-secondary);
}

.sidebar-nav {
  display: grid;
  gap: 8px;
}

.nav-link {
  display: flex;
  align-items: center;
  gap: 10px;
  text-decoration: none;
  color: var(--text-secondary);
  padding: 10px 12px;
  border-radius: 12px;
  border: 1px solid transparent;
  transition: background 0.2s, border-color 0.2s, color 0.2s;
}

.nav-link:hover,
.nav-link.active {
  color: var(--text-primary);
  background: var(--input-bg);
  border-color: var(--border-color);
}

.section-split {
  justify-content: space-between;
}

.icon-btn {
  width: 28px;
  height: 28px;
  border-radius: 999px;
  border: 1px solid var(--border-color);
  background: var(--input-bg);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.icon-btn:hover {
  color: var(--text-primary);
  border-color: var(--accent-color);
}

.conversation-list {
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  display: grid;
  gap: 8px;
}

.conversation-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: 1px solid transparent;
  border-radius: 12px;
  background: transparent;
  color: var(--text-secondary);
  padding: 10px 12px;
  cursor: pointer;
  text-align: left;
}

.conversation-item span {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.conversation-item:hover,
.conversation-item.active {
  background: var(--input-bg);
  color: var(--text-primary);
  border-color: var(--border-color);
}

.sidebar-footer {
  padding-top: 14px;
  margin-top: 14px;
  border-top: 1px solid var(--border-color);
}

.footer-btn {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border: 1px solid var(--border-color);
  background: transparent;
  color: var(--text-secondary);
  border-radius: 12px;
  padding: 10px 12px;
  cursor: pointer;
}

.footer-btn:hover {
  color: #fda4af;
  border-color: rgba(244, 63, 94, 0.35);
}
</style>
