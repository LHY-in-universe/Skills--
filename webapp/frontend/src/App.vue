<script setup>
import { ref, onMounted, provide, watch, nextTick } from 'vue'
import { RouterView } from 'vue-router'
import Sidebar from './components/Sidebar.vue'
import { createApiClient } from './lib/api'

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
const routingConfig = ref({ enabled: false, router_model: '', summary_model: '', tiers: { easy: '', medium: '', hard: '' } })
const lastRouteInfo = ref({ tier: '', model: '' })
const liveUsage = ref({ prompt: 0, completion: 0, total: 0, call_type: '', model: '', provider: '', cached_read: 0, cached_write: 0 })
const streamMeta = ref({ plan: [], currentStep: '', audit: '', auditReason: '', failover: [] })
const voiceRuntime = ref({
  enabled: false,
  convId: '',
  phase: 'idle',
  source: '',
  queueLength: 0,
  chunksReceived: 0,
  chunksPlayed: 0,
})

const isElectron = typeof window !== 'undefined' && !!window.electronAPI
const isCompact = ref(false)
const API_BASE = (isElectron && window.location.protocol === 'file:')
  ? 'http://localhost:8000'
  : ''
const api = createApiClient(API_BASE)
const notice = ref({ show: false, type: 'info', text: '' })
let noticeTimer = null

const permissionDialog = ref({
  visible: false,
  toolName: '',
  description: '',
})

let abortController = null

const notify = (text, type = 'info') => {
  notice.value = { show: true, type, text: String(text || '') }
  if (noticeTimer) clearTimeout(noticeTimer)
  noticeTimer = setTimeout(() => {
    notice.value.show = false
  }, 2600)
}

const toggleCompact = () => {
  isCompact.value = !isCompact.value
  if (isElectron) window.electronAPI.toggleCompact(isCompact.value)
}

watch(isLightMode, (val) => {
  if (val) document.body.classList.add('light-mode')
  else document.body.classList.remove('light-mode')
})

provide('messages', messages)
provide('isTyping', isTyping)
provide('streamingContent', streamingContent)
provide('isStreaming', isStreaming)
provide('streamingModel', streamingModel)
provide('models', models)
provide('currentModel', currentModel)
provide('skills', skills)
provide('apiConfig', apiConfig)
provide('isLightMode', isLightMode)
provide('permissionDialog', permissionDialog)
provide('conversations', conversations)
provide('activeConversationId', activeConversationId)
provide('apiBase', API_BASE)
provide('routingConfig', routingConfig)
provide('lastRouteInfo', lastRouteInfo)
provide('liveUsage', liveUsage)
provide('streamMeta', streamMeta)
provide('voiceRuntime', voiceRuntime)
provide('apiClient', api)
provide('notify', notify)

const fetchConfig = async () => {
  try {
    const configRes = await api.get('/api/config')
    if (configRes) {
      apiConfig.value = configRes
      currentModel.value = configRes.current_model
    }
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

provide('exportConversation', exportConversation)
provide('switchModel', switchModel)

const fetchHistory = async () => {
  try {
    const historyRes = await api.get('/api/history')
    const buckets = new Map()
    for (const msg of messages.value) {
      const key = `${msg?.role || ''}|${msg?.content || ''}|${JSON.stringify(msg?.tool_calls || [])}`
      const arr = buckets.get(key) || []
      if (msg && (msg._tokens || msg._model)) arr.push({ _tokens: msg._tokens, _model: msg._model })
      buckets.set(key, arr)
    }
    messages.value = historyRes.map((msg) => {
      const key = `${msg?.role || ''}|${msg?.content || ''}|${JSON.stringify(msg?.tool_calls || [])}`
      const arr = buckets.get(key) || []
      const cached = arr.shift()
      buckets.set(key, arr)
      return cached ? { ...msg, ...cached } : msg
    })
  } catch (err) {
    console.error('Failed to fetch history:', err)
  }
}

const fetchConversations = async () => {
  try {
    const res = await api.get('/api/conversations')
    conversations.value = res
    const active = res.find((item) => item.active)
    if (active) activeConversationId.value = active.id
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
  const safe = (promise) => promise.catch((err) => {
    console.error('fetch error:', err)
    return null
  })
  const [modelsRes, configRes, skillsRes, routingRes] = await Promise.all([
    safe(api.get('/api/models')),
    safe(api.get('/api/config')),
    safe(api.get('/api/skills')),
    safe(api.get('/api/routing')),
  ])
  if (modelsRes) {
    models.value = Object.entries(modelsRes).map(([displayName, config]) => ({
      displayName,
      apiId: config.id || config,
      provider: config.provider || '',
      apiUrl: config.api_url || '',
      enabled: config.enabled !== false,
      capabilities: config.capabilities || {},
      requires: config.requires || [],
    }))
  }
  if (configRes) {
    apiConfig.value = configRes
    currentModel.value = configRes.current_model
  }
  if (skillsRes) skills.value = skillsRes
  if (routingRes) routingConfig.value = routingRes
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
      if (!line.startsWith('data: ')) continue
      let event
      try {
        event = JSON.parse(line.slice(6))
      } catch {
        continue
      }

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
        case 'failover_step': {
          const arr = Array.isArray(streamMeta.value.failover) ? streamMeta.value.failover : []
          arr.push(event)
          streamMeta.value.failover = arr.slice(-8)
          break
        }
        case 'failover_exhausted': {
          const arr = Array.isArray(streamMeta.value.failover) ? streamMeta.value.failover : []
          arr.push({ ...event, failover_type: 'exhausted' })
          streamMeta.value.failover = arr.slice(-8)
          break
        }
        case 'start':
          streamingModel.value = event._model || currentModel.value
          apiConfig.value = {
            ...(apiConfig.value || {}),
            current_model: event._model || currentModel.value,
            effective_model_id: event._model_id || apiConfig.value?.effective_model_id || '',
            effective_provider: event._provider || apiConfig.value?.effective_provider || '',
          }
          break
        case 'text':
          streamingContent.value += event.content
          await nextTick()
          break
        case 'usage':
          pendingUsage = { prompt: event.prompt, completion: event.completion, total: event.total }
          liveUsage.value.prompt += event.prompt || 0
          liveUsage.value.completion += event.completion || 0
          liveUsage.value.total += event.total || ((event.prompt || 0) + (event.completion || 0))
          liveUsage.value.call_type = event.call_type || liveUsage.value.call_type || ''
          liveUsage.value.model = event.model || liveUsage.value.model || ''
          liveUsage.value.provider = event.provider || liveUsage.value.provider || ''
          liveUsage.value.cached_read += event.cached_read || 0
          liveUsage.value.cached_write += event.cached_write || 0
          break
        case 'permission_required':
          streamingContent.value = ''
          streamingModel.value = ''
          isStreaming.value = false
          permissionDialog.value = {
            visible: true,
            toolName: event.tool_name,
            description: event.description,
          }
          await fetchHistory()
          return { status: 'permission' }
        case 'aborted':
          streamingContent.value = ''
          streamingModel.value = ''
          isStreaming.value = false
          isTyping.value = false
          await fetchHistory()
          return { status: 'aborted' }
        case 'error':
          streamingContent.value = ''
          streamingModel.value = ''
          isStreaming.value = false
          isTyping.value = false
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
          streamingContent.value = ''
          streamingModel.value = ''
          isStreaming.value = false
          isTyping.value = false
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
    } catch {}
  }
  messages.value.push({ role: 'user', content: text })
  isTyping.value = true
  isStreaming.value = true
  streamingContent.value = ''
  streamMeta.value = { plan: [], currentStep: '', audit: '', auditReason: '', failover: [] }
  liveUsage.value = { prompt: 0, completion: 0, total: 0, call_type: '', model: '', provider: '', cached_read: 0, cached_write: 0 }
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
      streamingContent.value = ''
      streamingModel.value = ''
      isStreaming.value = false
      if (err.name !== 'AbortError') {
        messages.value.push({
          role: 'assistant',
          content: `**Error:** ${err.message}. Please check your API configuration.`,
        })
        isTyping.value = false
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
    streamingContent.value = ''
    streamingModel.value = ''
    isStreaming.value = false
    if (err.name !== 'AbortError') {
      messages.value.push({ role: 'assistant', content: `**Error:** ${err.message}` })
      isTyping.value = false
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

provide('clearHistoryFn', clearHistory)
provide('createConversationFn', createConversation)
provide('appActions', {
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
})

onMounted(fetchInitialData)
</script>

<template>
  <div v-if="isElectron && isCompact" class="desktop-bubble" @click="toggleCompact">🤖</div>

  <div v-if="isElectron && !isCompact" class="desktop-drag-bar">
    <span class="drag-region">桌面精灵</span>
    <button class="no-drag" @click="toggleCompact" title="折叠">—</button>
  </div>

  <div v-show="!isElectron || !isCompact" class="app-shell">
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
