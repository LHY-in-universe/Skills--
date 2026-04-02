<script setup>
import { ref, onMounted, provide } from 'vue'
import Sidebar from './components/Sidebar.vue'
import ChatContainer from './components/ChatContainer.vue'
import MessageInput from './components/MessageInput.vue'

const messages = ref([])
const isTyping = ref(false)
const models = ref({})
const currentModel = ref('')
const skills = ref([])
const apiConfig = ref({
  api_url: '',
  current_model: ''
})

// Provide state to children
provide('messages', messages)
provide('isTyping', isTyping)
provide('models', models)
provide('currentModel', currentModel)
provide('skills', skills)
provide('apiConfig', apiConfig)

const fetchInitialData = async () => {
  try {
    const [modelsRes, configRes, skillsRes, historyRes] = await Promise.all([
      fetch('/api/models').then(res => res.json()),
      fetch('/api/config').then(res => res.json()),
      fetch('/api/skills').then(res => res.json()),
      fetch('/api/history').then(res => res.json())
    ])
    
    models.value = modelsRes
    apiConfig.value = configRes
    currentModel.value = configRes.current_model
    skills.value = skillsRes
    messages.value = historyRes
  } catch (err) {
    console.error('Failed to fetch initial data:', err)
  }
}

const sendMessage = async (text) => {
  if (!text.trim()) return
  
  messages.value.push({ role: 'user', content: text })
  isTyping.value = true
  
  try {
    const response = await fetch('/api/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ user_input: text })
    })
    
    if (!response.ok) throw new Error('Failed to send message')
    
    const data = await response.json()
    messages.value.push(data)
  } catch (err) {
    messages.value.push({ 
      role: 'assistant', 
      content: `**Error:** ${err.message}. Please check your API configuration.` 
    })
  } finally {
    isTyping.value = false
  }
}

const clearHistory = async () => {
  await fetch('/api/history/clear', { method: 'POST' })
  messages.value = []
}

onMounted(fetchInitialData)
</script>

<template>
  <Sidebar 
    @clear-history="clearHistory"
    @refresh-data="fetchInitialData" 
  />
  <main class="chat-main">
    <ChatContainer />
    <MessageInput @send="sendMessage" />
  </main>
</template>
