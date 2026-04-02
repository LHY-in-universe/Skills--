<script setup>
import { inject, ref } from 'vue'
import { Cpu, Zap, Trash2, Globe, ChevronDown } from 'lucide-vue-next'

const emit = defineEmits(['clear-history', 'refresh-data', 'toggle-theme'])
const models = inject('models')
const currentModel = inject('currentModel')
const skills = inject('skills')
const apiConfig = inject('apiConfig')
const isLightMode = inject('isLightMode')

const customApiUrl = ref('')
const customApiKey = ref('')
const customModel = ref('')

const isManagingModels = ref(false)
const isApiConfigOpen = ref(false)   // collapsed by default
const newModelName = ref('')
const newModelId = ref('')

const toggleSkill = async (skill) => {
  try {
    await fetch('/api/skills/toggle', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: skill.name, enabled: !skill.enabled })
    })
    emit('refresh-data')
  } catch (err) {
    console.error('Failed to toggle skill:', err)
  }
}

const updateConfig = async () => {
  try {
    const response = await fetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        api_url: customApiUrl.value || undefined,
        api_key: customApiKey.value || undefined,
        model: customModel.value || undefined
      })
    })
    if (response.ok) {
      alert('Configuration updated successfully!')
      emit('refresh-data')
    }
  } catch (err) {
    alert('Error updating configuration: ' + err.message)
  }
}

const onModelChange = async (e) => {
  const val = e.target.value
  await fetch('/api/config', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model: val })
  })
  currentModel.value = val
}

const addModel = async () => {
  if (!newModelName.value || !newModelId.value) return
  try {
    const response = await fetch('/api/models', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: newModelName.value, model_id: newModelId.value })
    })
    if (response.ok) {
      newModelName.value = ''
      newModelId.value = ''
      emit('refresh-data')
    }
  } catch (err) {
    console.error('Failed to add model:', err)
  }
}

const deleteModel = async (name) => {
  if (!confirm(`Are you sure you want to delete ${name}?`)) return
  try {
    const response = await fetch(`/api/models/${name}`, { method: 'DELETE' })
    if (response.ok) emit('refresh-data')
  } catch (err) {
    console.error('Failed to delete model:', err)
  }
}
</script>

<template>
  <aside class="sidebar">
    <!-- Header -->
    <div class="sidebar-header">
      <h1 class="logo">SiliconFlow AI</h1>
      <button class="theme-toggle" @click="emit('toggle-theme')" :title="isLightMode ? '切换到暗色模式' : '切换到亮色模式'">
        {{ isLightMode ? '🌙' : '☀️' }}
      </button>
    </div>

    <!-- Models Section -->
    <div style="display: flex; align-items: center; justify-content: space-between; margin-top: 0.5rem; margin-bottom: 0.75rem;">
      <div class="section-title" style="margin: 0;"><Cpu size="14" style="margin-right: 5px;"/> SELECT MODEL</div>
      <button @click="isManagingModels = !isManagingModels" style="background: transparent; border: none; color: var(--accent-color); cursor: pointer; font-size: 11px; font-weight: 600;">{{ isManagingModels ? 'Done' : 'Manage' }}</button>
    </div>

    <!-- Model Selection / Management -->
    <div v-if="!isManagingModels">
      <select :value="currentModel" @change="onModelChange" style="width: 100%; background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); height: 42px; padding: 0 0.75rem; font-size: 13px; border-radius: 0.5rem; outline: none;">
        <option v-for="(name, id) in models" :key="id" :value="name">{{ id }} ({{ name }})</option>
      </select>
    </div>

    <div v-else class="config-group" style="background: var(--msg-assistant-bg); padding: 10px; border-radius: 8px; border: 1px solid var(--border-color);">
      <div v-for="(name, id) in models" :key="id" style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 5px; font-size: 12px; padding: 4px; border-bottom: 1px solid var(--border-color);">
        <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; color: var(--text-primary);">{{ id }}</span>
        <button @click="deleteModel(id)" style="background: transparent; border: none; color: #f87171; cursor: pointer; padding: 0 5px;">&times;</button>
      </div>
      <div style="margin-top: 10px; display: flex; flex-direction: column; gap: 5px;">
        <input v-model="newModelName" placeholder="Model Display Name" style="background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); padding: 6px; border-radius: 4px; font-size: 11px;" />
        <input v-model="newModelId" placeholder="Model API ID (e.g. deepseek-ai/DeepSeek-V3)" style="background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); padding: 6px; border-radius: 4px; font-size: 11px;" />
        <button @click="addModel" style="background: var(--accent-color); border: none; color: white; padding: 6px; border-radius: 4px; font-size: 11px; cursor: pointer;">Add Model</button>
      </div>
    </div>

    <!-- Skills Section -->
    <div class="section-title" style="margin-top: 1.5rem;"><Zap size="14" style="margin-right: 5px;"/> ACTIVE SKILLS</div>
    <div v-for="skill in skills" :key="skill.name" class="skill-toggle">
      <span style="font-size: 13px;">{{ skill.name }}</span>
      <label class="switch">
        <input type="checkbox" :checked="skill.enabled" @change="toggleSkill(skill)">
        <span class="slider"></span>
      </label>
    </div>

    <!-- Bottom Actions -->
    <div style="margin-top: auto; display: flex; gap: 10px; flex-direction: column; padding-top: 1rem;">

      <!-- API Config — collapsible at bottom -->
      <div class="collapsible-section">
        <button class="collapsible-header" @click="isApiConfigOpen = !isApiConfigOpen">
          <span style="display:flex;align-items:center;gap:6px;"><Globe size="13"/> API Configuration</span>
          <ChevronDown size="14" :style="{ transform: isApiConfigOpen ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s' }" />
        </button>
        <Transition name="collapse">
          <div v-if="isApiConfigOpen" class="collapsible-body">
            <div style="display: flex; gap: 6px; flex-direction: column;">
              <input
                v-model="customApiUrl"
                placeholder="API Base URL (Optional)"
                style="background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); padding: 8px; border-radius: 4px; font-size: 12px;"
              />
              <input
                v-model="customApiKey"
                type="password"
                placeholder="API Key / Token"
                style="background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); padding: 8px; border-radius: 4px; font-size: 12px;"
              />
              <input
                v-model="customModel"
                placeholder="Custom Model Name"
                style="background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); padding: 8px; border-radius: 4px; font-size: 12px;"
              />
              <button @click="updateConfig" style="background: var(--accent-color); color: white; border: none; padding: 8px; border-radius: 4px; cursor: pointer; font-size: 12px;">Update Config</button>
            </div>
          </div>
        </Transition>
      </div>

      <!-- Clear Conversation -->
      <button @click="emit('clear-history')" style="display: flex; align-items: center; justify-content: center; gap: 8px; width: 100%; background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.2); color: #f87171; padding: 12px; border-radius: 8px; cursor: pointer;">
        <Trash2 size="16"/> Clear Conversation
      </button>
    </div>
  </aside>
</template>

<style scoped>
.collapsible-section {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  overflow: hidden;
}

.collapsible-header {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  background: var(--msg-assistant-bg);
  border: none;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  transition: background 0.2s, color 0.2s;
}

.collapsible-header:hover {
  color: var(--text-primary);
  background: var(--input-bg);
}

.collapsible-body {
  padding: 10px 12px 12px;
  border-top: 1px solid var(--border-color);
  background: var(--panel-bg);
}

/* Collapse transition */
.collapse-enter-active,
.collapse-leave-active {
  transition: max-height 0.25s ease, opacity 0.2s ease;
  max-height: 300px;
  overflow: hidden;
}
.collapse-enter-from,
.collapse-leave-to {
  max-height: 0;
  opacity: 0;
}
</style>
