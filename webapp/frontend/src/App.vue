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

const fetchHistory = async () => {
  try {
    const historyRes = await fetch('/api/history').then(res => res.json())
    messages.value = historyRes
  } catch (err) {
    console.error('Failed to fetch history:', err)
  }
}

const fetchInitialData = async () => {
  try {
    const [modelsRes, configRes, skillsRes] = await Promise.all([
      fetch('/api/models').then(res => res.json()),
      fetch('/api/config').then(res => res.json()),
      fetch('/api/skills').then(res => res.json()),
    ])
    models.value = modelsRes
    apiConfig.value = configRes
    currentModel.value = configRes.current_model
    skills.value = skillsRes
    await fetchHistory()
  } catch (err) {
    console.error('Failed to fetch initial data:', err)
  }
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
    const response = await fetch('/api/chat', {
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

const handlePermissionResponse = async (granted) => {
  permissionDialog.value.visible = false
  abortController = new AbortController()
  try {
    const response = await fetch('/api/chat/resume', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ granted }),
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
  await fetch('/api/chat/abort', { method: 'POST' }).catch(() => {})
}

const clearHistory = async () => {
  await fetch('/api/history/clear', { method: 'POST' })
  messages.value = []
}

const toggleTheme = () => {
  isLightMode.value = !isLightMode.value
}

onMounted(fetchInitialData)
</script>

<template>
  <Sidebar
    @clear-history="clearHistory"
    @refresh-data="fetchInitialData"
    @toggle-theme="toggleTheme"
  />
  <main class="chat-main">
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
          <button class="perm-btn deny" @click="handlePermissionResponse(false)">
            ✕ 拒绝
          </button>
          <button class="perm-btn approve" @click="handlePermissionResponse(true)">
            ✓ 同意执行
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>
