<script setup>
import { ref, onMounted, provide, watch } from 'vue'
import Sidebar from './components/Sidebar.vue'
import ChatContainer from './components/ChatContainer.vue'
import MessageInput from './components/MessageInput.vue'

const messages = ref([])
const isTyping = ref(false)
const models = ref({})
const currentModel = ref('')
const skills = ref([])
const isLightMode = ref(false)
const apiConfig = ref({ api_url: '', current_model: '' })
const conversations = ref([])
const activeConversationId = ref('')

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
provide('models', models)
provide('currentModel', currentModel)
provide('skills', skills)
provide('apiConfig', apiConfig)
provide('isLightMode', isLightMode)
provide('permissionDialog', permissionDialog)
provide('conversations', conversations)
provide('activeConversationId', activeConversationId)
provide('apiBase', API_BASE)

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
  await Promise.all([fetchHistory(), fetchConversations()])
}

const createConversation = async () => {
  const res = await fetch(`${API_BASE}/api/conversations`, { method: 'POST' }).then(r => r.json())
  activeConversationId.value = res.id
  await Promise.all([fetchHistory(), fetchConversations()])
}

const fetchInitialData = async () => {
  const safe = (p) => p.catch(err => { console.error('fetch error:', err); return null })
  const [modelsRes, configRes, skillsRes] = await Promise.all([
    safe(fetch(`${API_BASE}/api/models`).then(r => r.json())),
    safe(fetch(`${API_BASE}/api/config`).then(r => r.json())),
    safe(fetch(`${API_BASE}/api/skills`).then(r => r.json())),
  ])
  if (modelsRes) models.value = modelsRes
  if (configRes) { apiConfig.value = configRes; currentModel.value = configRes.current_model }
  if (skillsRes) skills.value = skillsRes
  await Promise.all([fetchHistory(), fetchConversations()])
}

const handleApiResponse = async (data) => {
  if (data.type === 'permission_required') {
    // Show permission dialog, keep isTyping=true until resolved
    permissionDialog.value = {
      visible: true,
      toolName: data.tool_name,
      description: data.description
    }
  } else {
    // Refresh full history so intermediate tool call/result messages appear
    await fetchHistory()
    isTyping.value = false
  }
}

const sendMessage = async (text) => {
  if (!text.trim()) return
  messages.value.push({ role: 'user', content: text })
  isTyping.value = true

  abortController = new AbortController()
  try {
    const response = await fetch(`${API_BASE}/api/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ user_input: text }),
      signal: abortController.signal
    })
    if (!response.ok) throw new Error('Request failed')
    const data = await response.json()
    await handleApiResponse(data)
  } catch (err) {
    if (err.name === 'AbortError') {
      // Aborted by user — isTyping already set to false in abortChat()
    } else {
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
  abortController = new AbortController()
  try {
    const response = await fetch(`${API_BASE}/api/chat/resume`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ granted, always_allow: alwaysAllow }),
      signal: abortController.signal
    })
    if (!response.ok) throw new Error('Resume failed')
    const data = await response.json()
    await handleApiResponse(data)
  } catch (err) {
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
