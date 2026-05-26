<script setup>
import { ref, onMounted, provide, watch, nextTick } from 'vue'
import { RouterView } from 'vue-router'
import Sidebar from './components/Sidebar.vue'
import { createApiClient } from './lib/api'
import { provideAppContext } from './lib/appContext'
import {
  fetchConfigData,
  fetchConversationsData,
  fetchHistoryData,
  fetchInitialAppData,
} from './lib/chatData'
import {
  createLastRouteInfo,
  createLiveUsage,
  createNotice,
  createPermissionDialog,
  createRoutingConfig,
  createStreamMeta,
  createVoiceRuntime,
} from './lib/chatState'
import {
  applyStartEvent,
  applyUsageEvent,
  parseSseLine,
  pushFailoverEvent,
} from './lib/chatStream'

const messages = ref([])
const isTyping = ref(false)
const streamingContent = ref('')
const isStreaming = ref(false)
const streamingModel = ref('')
const models = ref([])
const currentModel = ref('')
const skills = ref([])
const isLightMode = ref(false)
const apiConfig = ref({ api_url: '', current_model: '' })
const conversations = ref([])
const activeConversationId = ref('')
const routingConfig = ref(createRoutingConfig())
const lastRouteInfo = ref(createLastRouteInfo())
const liveUsage = ref(createLiveUsage())
const streamMeta = ref(createStreamMeta())
const voiceRuntime = ref(createVoiceRuntime())

const API_BASE = ''
const api = createApiClient(API_BASE)
const notice = ref(createNotice())
let noticeTimer = null

const permissionDialog = ref(createPermissionDialog())

let abortController = null

const notify = (text, type = 'info') => {
  notice.value = { show: true, type, text: String(text || '') }
  if (noticeTimer) clearTimeout(noticeTimer)
  noticeTimer = setTimeout(() => {
    notice.value.show = false
  }, 2600)
}

watch(isLightMode, (val) => {
  if (val) document.body.classList.add('light-mode')
  else document.body.classList.remove('light-mode')
})

const resetStreamingState = ({ keepTyping = false } = {}) => {
  streamingContent.value = ''
  streamingModel.value = ''
  isStreaming.value = false
  if (!keepTyping) isTyping.value = false
}

const fetchConfig = async () => {
  try {
    await fetchConfigData({ api, apiConfig, currentModel })
  } catch (err) {
    console.error('Failed to fetch config:', err)
  }
}

const exportConversation = () => {
  const convName = conversations.value.find((item) => item.id === activeConversationId.value)?.name || '对话'
  const lines = [`# ${convName}\n`]
  for (const msg of messages.value) {
    if (msg.role === 'user') {
      lines.push(`**你**：${msg.content}\n`)
    } else if (msg.role === 'assistant' && msg.content?.trim()) {
      const model = msg._model ? ` (${msg._model.split('/').pop()})` : ''
      lines.push(`**AI${model}**：${msg.content}\n`)
    }
  }
  const blob = new Blob([lines.join('\n')], { type: 'text/markdown' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${convName.replace(/[/\\?%*:|"<>]/g, '-')}.md`
  a.click()
  URL.revokeObjectURL(url)
}

const switchModel = async (modelName) => {
  await api.post('/api/config', { model: modelName }).catch(() => {})
  await fetchConfig()
}

const fetchHistory = async () => {
  try {
    await fetchHistoryData({ api, messages })
  } catch (err) {
    console.error('Failed to fetch history:', err)
  }
}

const fetchConversations = async () => {
  try {
    await fetchConversationsData({ api, conversations, activeConversationId })
  } catch (err) {
    console.error('Failed to fetch conversations:', err)
  }
}

const switchConversation = async (convId) => {
  if (!convId || convId === activeConversationId.value) return
  isTyping.value = false
  await api.post(`/api/conversations/${convId}/activate`)
  activeConversationId.value = convId
  await Promise.all([fetchHistory(), fetchConversations(), fetchConfig()])
}

const createConversation = async () => {
  const res = await api.post('/api/conversations')
  activeConversationId.value = res.id
  await Promise.all([fetchHistory(), fetchConversations()])
}

const fetchInitialData = async () => {
  await fetchInitialAppData({
    api,
    models,
    apiConfig,
    currentModel,
    skills,
    routingConfig,
  })
  await Promise.all([fetchHistory(), fetchConversations()])
}

const processStream = async (response) => {
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  let respondingModel = currentModel.value
  let pendingUsage = null

  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() ?? ''

    for (const line of lines) {
      const event = parseSseLine(line)
      if (!event) continue

      switch (event.type) {
        case 'plan':
          streamMeta.value.plan = Array.isArray(event.steps) ? event.steps : []
          break
        case 'step_start':
          streamMeta.value.currentStep = event.step || ''
          break
        case 'step_done':
          streamMeta.value.currentStep = ''
          break
        case 'audit':
          streamMeta.value.audit = event.ok ? 'ok' : 'retry'
          if (event.ok === false && event.reason) streamMeta.value.auditReason = event.reason
          break
        case 'failover_step':
          pushFailoverEvent(streamMeta, event)
          break
        case 'failover_exhausted':
          pushFailoverEvent(streamMeta, event, true)
          break
        case 'start':
          applyStartEvent({ event, streamingModel, currentModel, apiConfig })
          break
        case 'text':
          streamingContent.value += event.content
          await nextTick()
          break
        case 'usage':
          pendingUsage = applyUsageEvent(liveUsage, event)
          break
        case 'permission_required':
          resetStreamingState({ keepTyping: true })
          permissionDialog.value = {
            visible: true,
            toolName: event.tool_name,
            description: event.description,
          }
          await fetchHistory()
          return { status: 'permission' }
        case 'aborted':
          resetStreamingState()
          await fetchHistory()
          return { status: 'aborted' }
        case 'error':
          resetStreamingState()
          return {
            status: 'error',
            message: event.content || 'Unknown stream error',
            errorClass: event.error_class || '',
          }
        case 'done': {
          respondingModel = event._model || streamingModel.value || currentModel.value
          if (event._tier) lastRouteInfo.value = { tier: event._tier, model: respondingModel }
          const prevLen = messages.value.length
          await fetchHistory()
          for (let i = prevLen; i < messages.value.length; i += 1) {
            const msg = messages.value[i]
            if (msg.role === 'assistant' && !msg.tool_calls && msg.content?.trim()) {
              messages.value[i] = {
                ...msg,
                _model: respondingModel,
                ...(pendingUsage ? { _tokens: pendingUsage } : {}),
              }
            }
          }
          pendingUsage = null
          resetStreamingState()
          break
        }
      }
    }
  }
  return { status: 'done' }
}

const sendMessage = async (text) => {
  if (!text.trim()) return
  if (abortController) {
    try {
      abortController.abort()
    } catch (err) {
      console.warn('Abort controller cleanup failed:', err)
    }
  }
  messages.value.push({ role: 'user', content: text })
  isTyping.value = true
  isStreaming.value = true
  streamingContent.value = ''
  streamMeta.value = createStreamMeta()
  liveUsage.value = createLiveUsage()
  let retried = false
  while (true) {
    abortController = new AbortController()
    try {
      const response = await api.stream('/api/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ user_input: text, conv_id: activeConversationId.value || undefined }),
        signal: abortController.signal,
      })
      const result = await processStream(response)
      if (result?.status === 'error') {
        const msg = String(result.message || '')
        const errClass = result.errorClass ? ` (${result.errorClass})` : ''
        const isConnErr = msg.toLowerCase().includes('connection error')
        if (isConnErr && !retried) {
          retried = true
          isTyping.value = true
          isStreaming.value = true
          streamingContent.value = ''
          continue
        }
        messages.value.push({ role: 'assistant', content: `**Error${errClass}:** ${msg}` })
      }
      break
    } catch (err) {
      resetStreamingState({ keepTyping: err.name === 'AbortError' })
      if (err.name !== 'AbortError') {
        messages.value.push({
          role: 'assistant',
          content: `**Error:** ${err.message}. Please check your API configuration.`,
        })
      }
      break
    }
  }
}

const handlePermissionResponse = async (granted, alwaysAllow = false) => {
  permissionDialog.value.visible = false
  isTyping.value = true
  isStreaming.value = true
  streamingContent.value = ''
  abortController = new AbortController()
  try {
    const response = await api.stream('/api/chat/resume', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        granted,
        always_allow: alwaysAllow,
        conv_id: activeConversationId.value || undefined,
      }),
      signal: abortController.signal,
    })
    const result = await processStream(response)
    if (result?.status === 'error') {
      messages.value.push({ role: 'assistant', content: `**Error:** ${result.message || 'Resume failed'}` })
    }
  } catch (err) {
    resetStreamingState({ keepTyping: err.name === 'AbortError' })
    if (err.name !== 'AbortError') {
      messages.value.push({ role: 'assistant', content: `**Error:** ${err.message}` })
    }
  }
}

const abortChat = async () => {
  if (abortController) abortController.abort()
  isTyping.value = false
  permissionDialog.value.visible = false
  await api.post('/api/chat/abort', { conv_id: activeConversationId.value || undefined }).catch(() => {})
}

const clearHistory = async () => {
  await api.post(`/api/history/clear?conv_id=${encodeURIComponent(activeConversationId.value || '')}`)
  messages.value = []
  await fetchConversations()
}

const toggleTheme = () => {
  isLightMode.value = !isLightMode.value
}

const appActions = {
  sendMessage,
  abortChat,
  refreshGlobalData: fetchInitialData,
  fetchConfig,
  fetchHistory,
  fetchConversations,
  switchConversation,
  createConversation,
  clearHistory,
  toggleTheme,
}

provideAppContext(provide, {
  messages,
  isTyping,
  streamingContent,
  isStreaming,
  streamingModel,
  models,
  currentModel,
  skills,
  apiConfig,
  isLightMode,
  permissionDialog,
  conversations,
  activeConversationId,
  apiBase: API_BASE,
  routingConfig,
  lastRouteInfo,
  liveUsage,
  streamMeta,
  voiceRuntime,
  api,
  notify,
  exportConversation,
  switchModel,
  clearHistory,
  createConversation,
  appActions,
})

onMounted(fetchInitialData)
</script>

<template>
  <div class="app-shell">
    <Sidebar />
    <main class="chat-main app-content">
      <div v-if="notice.show" class="global-notice" :class="`notice-${notice.type}`">{{ notice.text }}</div>
      <RouterView />
    </main>
  </div>
  <Transition name="dialog-fade">
    <div v-if="permissionDialog.visible" class="permission-overlay" @click.self="handlePermissionResponse(false)">
      <div class="permission-dialog">
        <div class="permission-header">
          <span class="permission-icon">🔐</span>
          <div>
            <h3>权限请求</h3>
            <p class="permission-tool">工具: <code>{{ permissionDialog.toolName }}</code></p>
          </div>
        </div>
        <div class="permission-body">
          <p>AI 请求执行以下操作：</p>
          <pre class="permission-detail">{{ permissionDialog.description }}</pre>
        </div>
        <div class="permission-actions">
          <button class="perm-btn always" @click="handlePermissionResponse(true, true)">∞ 一直同意</button>
          <button class="perm-btn deny" @click="handlePermissionResponse(false)">✕ 拒绝</button>
          <button class="perm-btn approve" @click="handlePermissionResponse(true, false)">✓ 同意执行</button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.app-shell {
  display: flex;
  width: 100%;
  height: 100%;
}

.app-content {
  min-width: 0;
}
</style>
