<script setup>
import { inject, ref, defineEmits } from 'vue'
import { Settings, Cpu, Zap, Cloud, Trash2, Key, Globe, Layout } from 'lucide-vue-next'

const emit = defineEmits(['clear-history', 'refresh-data'])
const models = inject('models')
const currentModel = inject('currentModel')
const skills = inject('skills')
const apiConfig = inject('apiConfig')

const customApiUrl = ref('')
const customApiKey = ref('')
const customModel = ref('')

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

</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header">
      <h1 class="logo">SiliconFlow AI</h1>
    </div>

    <!-- API Config Section -->
    <div class="section-title"><Globe size="14" style="margin-right: 5px;"/> API CONFIGURATION</div>
    <div class="config-group" style="margin-bottom: 20px;">
      <div style="display: flex; gap: 5px; flex-direction: column;">
        <input 
          v-model="customApiUrl" 
          placeholder="API Base URL (Optional)" 
          style="background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1); color: white; padding: 8px; border-radius: 4px; font-size: 12px;"
        />
        <input 
          v-model="customApiKey" 
          type="password"
          placeholder="API Key / Token" 
          style="background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1); color: white; padding: 8px; border-radius: 4px; font-size: 12px;"
        />
        <input 
          v-model="customModel" 
          placeholder="Custom Model Name" 
          style="background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1); color: white; padding: 8px; border-radius: 4px; font-size: 12px;"
        />
        <button @click="updateConfig" style="background: var(--accent-color); color: white; border: none; padding: 8px; border-radius: 4px; cursor: pointer; font-size: 12px; margin-top: 5px;">Update Config</button>
      </div>
    </div>

    <!-- Models Section -->
    <div class="section-title"><Cpu size="14" style="margin-right: 5px;"/> SELECT MODEL</div>
    <select :value="currentModel" @change="onModelChange" style="width: 100%; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1); color: white; padding: 10px; border-radius: 8px; outline: none;">
      <option v-for="(name, id) in models" :key="id" :value="name">{{ name }}</option>
    </select>

    <!-- Skills Section -->
    <div class="section-title"><Zap size="14" style="margin-right: 5px;"/> ACTIVE SKILLS</div>
    <div v-for="skill in skills" :key="skill.name" class="skill-toggle">
      <span style="font-size: 13px;">{{ skill.name }}</span>
      <label class="switch">
        <input type="checkbox" :checked="skill.enabled" @change="toggleSkill(skill)">
        <span class="slider"></span>
      </label>
    </div>

    <!-- Reset Data -->
    <div style="margin-top: auto; display: flex; gap: 10px; flex-direction: column;">
        <button @click="emit('clear-history')" style="display: flex; align-items: center; justify-content: center; gap: 8px; width: 100%; background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.2); color: #f87171; padding: 12px; border-radius: 8px; cursor: pointer;">
            <Trash2 size="16"/> Clear Conversation
        </button>
    </div>
  </aside>
</template>
