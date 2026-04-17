<script setup>
import { ref, onMounted, provide, watch, nextTick } from 'vue'
import Sidebar from './components/Sidebar.vue'
import ChatContainer from './components/ChatContainer.vue'
import MessageInput from './components/MessageInput.vue'
import VoiceAssistant from './components/VoiceAssistant.vue'
import { createApiClient } from './lib/api'

const messages = ref([])
const isTyping = ref(false)
const streamingContent = ref('')
const isStreaming = ref(false)
const streamingModel = ref('')  // model name shown during generation
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

// Desktop sprite (Electron) mode
const isElectron = typeof window !== 'undefined' && !!window.electronAPI
const isCompact = ref(false)
// Dev mode: Electron loads from http://localhost:5173, Vite proxy works, use relative URLs
// Prod mode: Electron loads from file://, no proxy, need absolute URL
const API_BASE = (isElectron && window.location.protocol === 'file:')
  ? 'http://localhost:8000'
  : ''
const api = createApiClient(API_BASE)
const notice = ref({ show: false, type: 'info', text: '' })
let noticeTimer = null

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

// Permission dialog state
const permissionDialog = ref({
  visible: false,
  toolName: '',
  description: ''
})

// Abort controller ref for canceling fetch
let abortController = null

// Sync light mode to document.body so all CSS vars take effect globally
watch(isLightMode, (val) => {
  if (val) {
    document.body.classList.add('light-mode')
  } else {
    document.body.classList.remove('light-mode')
  }
})

// Provide state to children
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
    if (configRes) { apiConfig.value = configRes; currentModel.value = configRes.current_model }
  } catch (err) {
    console.error('Failed to fetch config:', err)
  }
}

const exportConversation = () => {
  const convName = conversations.value.find(c => c.id === activeConversationId.value)?.name || '对话'
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
    for (const m of messages.value) {
      const key = `${m?.role || ''}|${m?.content || ''}|${JSON.stringify(m?.tool_calls || [])}`
      const arr = buckets.get(key) || []
      if (m && (m._tokens || m._model)) arr.push({ _tokens: m._tokens, _model: m._model })
      buckets.set(key, arr)
    }
    messages.value = historyRes.map((m) => {
      const key = `${m?.role || ''}|${m?.content || ''}|${JSON.stringify(m?.tool_calls || [])}`
      const arr = buckets.get(key) || []
      const cached = arr.shift()
      buckets.set(key, arr)
      return cached ? { ...m, ...cached } : m
    })
  } catch (err) {
    console.error('Failed to fetch history:', err)
  }
}

const fetchConversations = async () => {
  try {
    const res = await api.get('/api/conversations')
    conversations.value = res
    const active = res.find(c => c.active)
    if (active) activeConversationId.value = active.id
  } catch (err) {
    console.error('Failed to fetch conversations:', err)
  }
}

const switchConversation = async (convId) => {
  if (convId === activeConversationId.value) return
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
  const safe = (p) => p.catch(err => { console.error('fetch error:', err); return null })
  const [modelsRes, configRes, skillsRes, routingRes] = await Promise.all([
    safe(api.get('/api/models')),
    safe(api.get('/api/config')),
    safe(api.get('/api/skills')),
    safe(api.get('/api/routing')),
  ])
  if (modelsRes) {
    // Transform dict { "Display Name": { id, provider, api_url } } to array of objects
    models.value = Object.entries(modelsRes).map(([displayName, config]) => ({
      displayName,
      apiId: config.id || config, // handle legacy string values if any
      provider: config.provider || '',
      apiUrl: config.api_url || '',
      enabled: config.enabled !== false,
      capabilities: config.capabilities || {},
      requires: config.requires || [],
    }))
  }
  if (configRes) { apiConfig.value = configRes; currentModel.value = configRes.current_model }
  if (skillsRes) skills.value = skillsRes
  if (routingRes) routingConfig.value = routingRes
  await Promise.all([fetchHistory(), fetchConversations()])
}

const sidebarRef = ref(null)

// ── SSE stream reader ────────────────────────────────────────
const processStream = async (response) => {
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  let respondingModel = currentModel.value
  let _pendingUsage = null

  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() ?? ''

    for (const line of lines) {
      if (!line.startsWith('data: ')) continue
      let event
      try { event = JSON.parse(line.slice(6)) } catch { continue }

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
            effective_provider: event._provider || apiConfig.value?.effective_provider || ''
          }
          break

        case 'text':
          streamingContent.value += event.content
          await nextTick()  // yield to browser for each chunk so streaming is visible
          break

        case 'tool_start':
          // tool indicator shown via isTyping dots; history refresh on done will show full record
          break

        case 'tool_done':
          break

        case 'usage':
          _pendingUsage = { prompt: event.prompt, completion: event.completion, total: event.total }
          liveUsage.value.prompt += event.prompt || 0
          liveUsage.value.completion += event.completion || 0
          liveUsage.value.total += event.total || ((event.prompt || 0) + (event.completion || 0))
          liveUsage.value.call_type = event.call_type || liveUsage.value.call_type || ''
          liveUsage.value.model = event.model || liveUsage.value.model || ''
          liveUsage.value.provider = event.provider || liveUsage.value.provider || ''
          liveUsage.value.cached_read += event.cached_read || 0
          liveUsage.value.cached_write += event.cached_write || 0
          sidebarRef.value?.fetchTokenStats()
          break

        case 'permission_required':
          streamingContent.value = ''
          streamingModel.value = ''
          isStreaming.value = false
          permissionDialog.value = {
            visible: true,
            toolName: event.tool_name,
            description: event.description
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
            errorClass: event.error_class || ''
          }

        case 'done': {
          respondingModel = event._model || streamingModel.value || currentModel.value
          if (event._tier) {
            lastRouteInfo.value = { tier: event._tier, model: respondingModel }
          }
          // Fetch history first so the final message is ready,
          // then clear streaming content — avoids flash of empty space
          const prevLen = messages.value.length
          await fetchHistory()
          for (let i = prevLen; i < messages.value.length; i++) {
            const msg = messages.value[i]
            if (msg.role === 'assistant' && !msg.tool_calls && msg.content?.trim()) {
              messages.value[i] = {
                ...msg,
                _model: respondingModel,
                ...(_pendingUsage ? { _tokens: _pendingUsage } : {})
              }
            }
          }
          _pendingUsage = null
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
    try { abortController.abort() } catch {}
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
        signal: abortController.signal
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
          content: `**Error:** ${err.message}. Please check your API configuration.`
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
        conv_id: activeConversationId.value || undefined
      }),
      signal: abortController.signal
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
  await api.post('/api/history/clear?conv_id=' + encodeURIComponent(activeConversationId.value || ''))
  messages.value = []
  await fetchConversations()
}

const toggleTheme = () => {
  isLightMode.value = !isLightMode.value
}

provide('clearHistoryFn', clearHistory)
provide('createConversationFn', createConversation)

onMounted(fetchInitialData)
</script>

<template>
  <!-- Desktop sprite: compact bubble -->
  <div v-if="isElectron && isCompact" class="desktop-bubble" @click="toggleCompact">🤖</div>

  <!-- Desktop sprite: drag bar (expanded mode) -->
  <div v-if="isElectron && !isCompact" class="desktop-drag-bar">
    <span class="drag-region">桌面精灵</span>
    <button class="no-drag" @click="toggleCompact" title="折叠">—</button>
  </div>

  <Sidebar
    ref="sidebarRef"
    v-show="!isElectron || !isCompact"
    @clear-history="clearHistory"
    @refresh-data="fetchInitialData"
    @toggle-theme="toggleTheme"
    @switch-conversation="switchConversation"
    @create-conversation="createConversation"
  />
  <main v-show="!isElectron || !isCompact" class="chat-main">
    <div v-if="notice.show" class="global-notice" :class="`notice-${notice.type}`">{{ notice.text }}</div>
    <ChatContainer />
    <MessageInput @send="sendMessage" @abort="abortChat" />
  </main>
  
  <VoiceAssistant />

  <!-- Permission Dialog -->
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
          <button class="perm-btn always" @click="handlePermissionResponse(true, true)">
            ∞ 一直同意
          </button>
          <button class="perm-btn deny" @click="handlePermissionResponse(false)">
            ✕ 拒绝
          </button>
          <button class="perm-btn approve" @click="handlePermissionResponse(true, false)">
            ✓ 同意执行
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.global-notice {
  position: sticky;
  top: 8px;
  z-index: 20;
  margin: 8px auto 0;
  width: fit-content;
  max-width: 92%;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 600;
  border: 1px solid var(--border-color);
  background: var(--msg-assistant-bg);
  color: var(--text-primary);
}
.notice-success { border-color: #10b98166; }
.notice-error { border-color: #ef444466; }
</style>
