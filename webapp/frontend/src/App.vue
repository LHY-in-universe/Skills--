<script setup>
import { ref, onMounted, provide, watch, nextTick } from 'vue'
import Sidebar from './components/Sidebar.vue'
import ChatContainer from './components/ChatContainer.vue'
import MessageInput from './components/MessageInput.vue'
import VoiceAssistant from './components/VoiceAssistant.vue'

const messages = ref([])
const isTyping = ref(false)
const streamingContent = ref('')
const isStreaming = ref(false)
const streamingModel = ref('')  // model name shown during generation
const models = ref({})
const currentModel = ref('')
const skills = ref([])
const isLightMode = ref(false)
const apiConfig = ref({ api_url: '', current_model: '' })
const conversations = ref([])
const activeConversationId = ref('')
const routingConfig = ref({ enabled: false, router_model: '', summary_model: '', tiers: { easy: '', medium: '', hard: '' } })
const lastRouteInfo = ref({ tier: '', model: '' })

// Desktop sprite (Electron) mode
const isElectron = typeof window !== 'undefined' && !!window.electronAPI
const isCompact = ref(false)
// Dev mode: Electron loads from http://localhost:5173, Vite proxy works, use relative URLs
// Prod mode: Electron loads from file://, no proxy, need absolute URL
const API_BASE = (isElectron && window.location.protocol === 'file:')
  ? 'http://localhost:8000'
  : ''

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
const fetchConfig = async () => {
  try {
    const configRes = await fetch(`${API_BASE}/api/config`).then(r => r.json())
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

const switchModel = async (modelId) => {
  await fetch(`${API_BASE}/api/config`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model: modelId })
  }).catch(() => {})
  currentModel.value = modelId
}

provide('exportConversation', exportConversation)
provide('switchModel', switchModel)

const fetchHistory = async () => {
  try {
    const historyRes = await fetch(`${API_BASE}/api/history`).then(res => res.json())
    messages.value = historyRes
  } catch (err) {
    console.error('Failed to fetch history:', err)
  }
}

const fetchConversations = async () => {
  try {
    const res = await fetch(`${API_BASE}/api/conversations`).then(r => r.json())
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
  await fetch(`${API_BASE}/api/conversations/${convId}/activate`, { method: 'POST' })
  activeConversationId.value = convId
  await Promise.all([fetchHistory(), fetchConversations(), fetchConfig()])
}

const createConversation = async () => {
  const res = await fetch(`${API_BASE}/api/conversations`, { method: 'POST' }).then(r => r.json())
  activeConversationId.value = res.id
  await Promise.all([fetchHistory(), fetchConversations()])
}

const fetchInitialData = async () => {
  const safe = (p) => p.catch(err => { console.error('fetch error:', err); return null })
  const [modelsRes, configRes, skillsRes, routingRes] = await Promise.all([
    safe(fetch(`${API_BASE}/api/models`).then(r => r.json())),
    safe(fetch(`${API_BASE}/api/config`).then(r => r.json())),
    safe(fetch(`${API_BASE}/api/skills`).then(r => r.json())),
    safe(fetch(`${API_BASE}/api/routing`).then(r => r.json())),
  ])
  if (modelsRes) models.value = modelsRes
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
        case 'start':
          streamingModel.value = event._model || currentModel.value
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
          return

        case 'aborted':
          streamingContent.value = ''
          streamingModel.value = ''
          isStreaming.value = false
          isTyping.value = false
          await fetchHistory()
          return

        case 'error':
          streamingContent.value = ''
          streamingModel.value = ''
          isStreaming.value = false
          isTyping.value = false
          messages.value.push({ role: 'assistant', content: `**Error:** ${event.content}` })
          return

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
}

const sendMessage = async (text) => {
  if (!text.trim()) return
  messages.value.push({ role: 'user', content: text })
  isTyping.value = true
  isStreaming.value = true
  streamingContent.value = ''

  abortController = new AbortController()
  try {
    const response = await fetch(`${API_BASE}/api/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ user_input: text }),
      signal: abortController.signal
    })
    if (!response.ok) throw new Error('Request failed')
    await processStream(response)
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
  }
}

const handlePermissionResponse = async (granted, alwaysAllow = false) => {
  permissionDialog.value.visible = false
  isTyping.value = true
  isStreaming.value = true
  streamingContent.value = ''

  abortController = new AbortController()
  try {
    const response = await fetch(`${API_BASE}/api/chat/resume`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ granted, always_allow: alwaysAllow }),
      signal: abortController.signal
    })
    if (!response.ok) throw new Error('Resume failed')
    await processStream(response)
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
  await fetch(`${API_BASE}/api/chat/abort`, { method: 'POST' }).catch(() => {})
}

const clearHistory = async () => {
  await fetch(`${API_BASE}/api/history/clear`, { method: 'POST' })
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
