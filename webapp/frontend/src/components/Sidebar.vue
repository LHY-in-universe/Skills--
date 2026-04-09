<script setup>
import { inject, ref, computed, onMounted, defineExpose } from 'vue'
import { Cpu, Zap, Trash2, Globe, ChevronDown, FolderOpen, MessageSquarePlus, MessagesSquare, GitBranch, BarChart2 } from 'lucide-vue-next'

const emit = defineEmits(['clear-history', 'refresh-data', 'toggle-theme', 'switch-conversation', 'create-conversation'])
const models = inject('models')
const currentModel = inject('currentModel')
const skills = inject('skills')
const apiConfig = inject('apiConfig')
const isLightMode = inject('isLightMode')
const conversationsRef = inject('conversations')
const activeConversationIdRef = inject('activeConversationId')
const apiBase = inject('apiBase', '')
const routingConfig = inject('routingConfig')

const customApiUrl = ref('')
const customApiKey = ref('')
const customModel = ref('')

const isManagingModels = ref(false)
const isApiConfigOpen = ref(false)   // collapsed by default
const isSkillsOpen = ref(true)       // expanded by default
const newModelName = ref('')
const newModelId = ref('')

// Conversation management
const editingConvId = ref(null)
const editingConvName = ref('')

const startRename = (conv) => {
  editingConvId.value = conv.id
  editingConvName.value = conv.name
}

const confirmRename = async (convId) => {
  if (!editingConvName.value.trim()) return cancelRename()
  await fetch(`${apiBase}/api/conversations/${convId}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name: editingConvName.value })
  })
  editingConvId.value = null
  emit('refresh-data')
}

const cancelRename = () => { editingConvId.value = null }

const deleteConversation = async (convId) => {
  await fetch(`${apiBase}/api/conversations/${convId}`, { method: 'DELETE' })
  emit('refresh-data')
}

const toggleSkill = async (skill) => {
  try {
    await fetch(`${apiBase}/api/skills/toggle`, {
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
    const response = await fetch(`${apiBase}/api/config`, {
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
  await fetch(`${apiBase}/api/config`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model: val })
  })
  currentModel.value = val
}

const addModel = async () => {
  if (!newModelName.value || !newModelId.value) return
  try {
    const response = await fetch(`${apiBase}/api/models`, {
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
    const response = await fetch(`${apiBase}/api/models/${name}`, { method: 'DELETE' })
    if (response.ok) emit('refresh-data')
  } catch (err) {
    console.error('Failed to delete model:', err)
  }
}

// Terminal working directory
const terminalCwd = ref('')
const cwdSaveStatus = ref('')  // '', 'saved', 'error'

const loadTerminalCwd = async () => {
  try {
    const res = await fetch(`${apiBase}/api/terminal/cwd`).then(r => r.json())
    terminalCwd.value = res.cwd || ''
  } catch (err) {
    console.error('Failed to load terminal cwd:', err)
  }
}

const saveTerminalCwd = async () => {
  try {
    const res = await fetch(`${apiBase}/api/terminal/cwd`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ cwd: terminalCwd.value })
    })
    if (res.ok) {
      const data = await res.json()
      terminalCwd.value = data.cwd || ''
      cwdSaveStatus.value = 'saved'
      setTimeout(() => { cwdSaveStatus.value = '' }, 2000)
    } else {
      const err = await res.json()
      cwdSaveStatus.value = 'error'
      alert(err.detail || '目录无效')
      setTimeout(() => { cwdSaveStatus.value = '' }, 2000)
    }
  } catch (err) {
    cwdSaveStatus.value = 'error'
    setTimeout(() => { cwdSaveStatus.value = '' }, 2000)
  }
}

// Routing config
const isRoutingOpen = ref(false)
const routingSaveStatus = ref('')

const saveRouting = async () => {
  try {
    const res = await fetch(`${apiBase}/api/routing`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(routingConfig.value)
    })
    if (res.ok) {
      routingSaveStatus.value = 'saved'
      setTimeout(() => { routingSaveStatus.value = '' }, 2000)
    } else {
      routingSaveStatus.value = 'error'
      setTimeout(() => { routingSaveStatus.value = '' }, 2000)
    }
  } catch {
    routingSaveStatus.value = 'error'
    setTimeout(() => { routingSaveStatus.value = '' }, 2000)
  }
}

// Token usage stats
const tokenStats = ref(null)
const isTokenOpen = ref(false)

const todayTokens = computed(() => {
  const today = new Date().toISOString().slice(0, 10)
  return tokenStats.value?.daily?.[today]?.total ?? 0
})

const TYPE_LABELS = {
  chat: '对话', skill: 'Skill 调用', router: '路由',
  compress: '压缩', embedding_ctx: '向量(上下文)', embedding_mem: '向量(记忆)'
}

const fetchTokenStats = async () => {
  try {
    const r = await fetch(`${apiBase}/api/token-usage`)
    tokenStats.value = await r.json()
  } catch {}
}

defineExpose({ fetchTokenStats })

onMounted(() => { loadTerminalCwd(); fetchTokenStats() })
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

    <!-- Scrollable content area -->
    <div class="sidebar-scroll">

    <!-- Conversations Section -->
    <div class="section-title" style="margin-top: 0.5rem;">
      <MessagesSquare size="14" style="margin-right: 5px;"/>
      对话列表
      <button class="conv-new-btn" @click="emit('create-conversation')" title="新建对话">
        <MessageSquarePlus size="14" />
      </button>
    </div>
    <div class="conv-list">
      <div
        v-for="conv in conversationsRef"
        :key="conv.id"
        class="conv-item"
        :class="{ active: conv.id === activeConversationIdRef }"
        @click="emit('switch-conversation', conv.id)"
      >
        <template v-if="editingConvId === conv.id">
          <input
            class="conv-rename-input"
            v-model="editingConvName"
            @keydown.enter="confirmRename(conv.id)"
            @keydown.esc="cancelRename"
            @click.stop
            autofocus
          />
          <button class="conv-action-btn" @click.stop="confirmRename(conv.id)" title="确认">✓</button>
        </template>
        <template v-else>
          <span class="conv-name" @dblclick.stop="startRename(conv)">{{ conv.name }}</span>
          <span class="conv-count">{{ conv.message_count }}</span>
          <button class="conv-action-btn del" @click.stop="deleteConversation(conv.id)" title="删除">×</button>
        </template>
      </div>
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
    <div class="collapsible-section" style="margin-top: 1.5rem;">
      <button class="collapsible-header" @click="isSkillsOpen = !isSkillsOpen">
        <span style="display:flex;align-items:center;gap:6px;">
          <Zap size="13"/> ACTIVE SKILLS
          <span style="font-size:10px;color:var(--text-secondary);font-weight:500;">({{ skills.length }})</span>
        </span>
        <ChevronDown size="14" :style="{ transform: isSkillsOpen ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s' }" />
      </button>
      <Transition name="collapse">
        <div v-if="isSkillsOpen" class="collapsible-body" style="padding: 8px 10px 10px;">
          <div class="skills-scroll">
            <div v-for="skill in skills" :key="skill.name" class="skill-toggle">
              <span style="font-size: 13px;">{{ skill.name }}</span>
              <label class="switch">
                <input type="checkbox" :checked="skill.enabled" @change="toggleSkill(skill)">
                <span class="slider"></span>
              </label>
            </div>
          </div>
        </div>
      </Transition>
    </div>

    <!-- Token Usage Section -->
    <div class="collapsible-section" style="margin-top: 1.5rem;" v-if="tokenStats">
      <button class="collapsible-header" @click="isTokenOpen = !isTokenOpen">
        <span style="display:flex;align-items:center;gap:6px;">
          <BarChart2 size="13"/> TOKEN USAGE
          <span style="font-size:10px;color:var(--text-secondary);font-weight:500;">今日 {{ todayTokens.toLocaleString() }}</span>
        </span>
        <ChevronDown size="14" :style="{ transform: isTokenOpen ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s' }" />
      </button>
      <Transition name="collapse">
        <div v-if="isTokenOpen" class="collapsible-body token-body">
          <div class="token-summary">
            <div class="token-summary-item">
              <span class="token-summary-label">今日</span>
              <span class="token-summary-val">{{ todayTokens.toLocaleString() }}</span>
            </div>
            <div class="token-summary-sep">·</div>
            <div class="token-summary-item">
              <span class="token-summary-label">累计</span>
              <span class="token-summary-val">{{ tokenStats.global.total.toLocaleString() }}</span>
            </div>
            <div class="token-summary-sep">·</div>
            <div class="token-summary-item">
              <span class="token-summary-label">调用</span>
              <span class="token-summary-val">{{ tokenStats.global.calls }}</span>
            </div>
          </div>
          <div class="token-breakdown">
            <template v-for="(v, k) in tokenStats.global.by_type" :key="k">
              <div v-if="v.calls > 0" class="token-row">
                <div class="token-row-top">
                  <span class="token-type-badge">{{ TYPE_LABELS[k] ?? k }}</span>
                  <span class="token-row-calls">×{{ v.calls }}</span>
                </div>
                <div class="token-row-detail">
                  <span class="token-detail-item prompt" title="输入 tokens">↑ {{ v.prompt.toLocaleString() }}</span>
                  <span class="token-detail-sep">+</span>
                  <span class="token-detail-item completion" title="输出 tokens">↓ {{ v.completion.toLocaleString() }}</span>
                  <span class="token-detail-sep">=</span>
                  <span class="token-detail-total">{{ v.total.toLocaleString() }}</span>
                </div>
              </div>
            </template>
          </div>
        </div>
      </Transition>
    </div>

    <!-- Route Strategy Section -->
    <div class="collapsible-section" style="margin-top: 1.5rem;">
      <button class="collapsible-header" @click="isRoutingOpen = !isRoutingOpen">
        <span style="display:flex;align-items:center;gap:6px;">
          <GitBranch size="13"/> ROUTE STRATEGY
          <span v-if="routingConfig?.enabled" style="font-size:10px;background:rgba(99,102,241,0.2);color:var(--accent-color);padding:1px 6px;border-radius:10px;font-weight:700;">ON</span>
        </span>
        <ChevronDown size="14" :style="{ transform: isRoutingOpen ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s' }" />
      </button>
      <Transition name="collapse">
        <div v-if="isRoutingOpen" class="collapsible-body">
          <!-- Enable toggle -->
          <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:12px;">
            <span style="font-size:12px;color:var(--text-secondary);">启用模型路由</span>
            <label class="switch">
              <input type="checkbox" :checked="routingConfig?.enabled" @change="e => { routingConfig.enabled = e.target.checked; saveRouting() }">
              <span class="slider"></span>
            </label>
          </div>
          <!-- Router (classifier) model -->
          <div style="margin-bottom:10px;">
            <div style="font-size:11px;color:var(--text-secondary);margin-bottom:4px;letter-spacing:0.04em;">分类器模型（小模型）</div>
            <select v-model="routingConfig.router_model" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:34px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;">
              <option value="">— 未设置 —</option>
              <option v-for="(apiId, displayName) in models" :key="apiId" :value="apiId">{{ displayName }}</option>
              <option v-if="routingConfig.router_model && !Object.values(models).includes(routingConfig.router_model)" :value="routingConfig.router_model">{{ routingConfig.router_model }}</option>
            </select>
          </div>
          <!-- Summary model -->
          <div style="margin-bottom:10px;">
            <div style="font-size:11px;color:var(--text-secondary);margin-bottom:4px;letter-spacing:0.04em;">摘要压缩模型</div>
            <select v-model="routingConfig.summary_model" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:34px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;">
              <option value="">— 未设置（使用当前主模型） —</option>
              <option v-for="(apiId, displayName) in models" :key="apiId" :value="apiId">{{ displayName }}</option>
              <option v-if="routingConfig.summary_model && !Object.values(models).includes(routingConfig.summary_model)" :value="routingConfig.summary_model">{{ routingConfig.summary_model }}</option>
            </select>
            <div style="font-size:10px;color:var(--text-secondary);opacity:0.6;margin-top:3px;">用于对话历史压缩，推荐小模型</div>
          </div>
          <!-- Tier models -->
          <div style="display:flex;flex-direction:column;gap:7px;margin-bottom:12px;">
            <div v-for="(label, tier) in { easy: '🟢 简单', medium: '🟡 中等', hard: '🔴 困难' }" :key="tier">
              <div style="font-size:11px;color:var(--text-secondary);margin-bottom:3px;">{{ label }}</div>
              <select v-model="routingConfig.tiers[tier]" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:32px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;">
                <option value="">— 未设置（使用默认） —</option>
                <option v-for="(apiId, displayName) in models" :key="apiId" :value="apiId">{{ displayName }}</option>
              </select>
            </div>
          </div>
          <!-- Save button -->
          <button
            @click="saveRouting"
            :style="{
              width: '100%',
              background: routingSaveStatus === 'saved' ? '#10b981' : routingSaveStatus === 'error' ? '#ef4444' : 'var(--accent-color)',
              border: 'none', color: 'white', padding: '7px', borderRadius: '5px',
              cursor: 'pointer', fontSize: '12px', fontWeight: '600', transition: 'background 0.2s'
            }"
          >
            {{ routingSaveStatus === 'saved' ? '✓ 已保存' : routingSaveStatus === 'error' ? '✕ 保存失败' : '保存路由配置' }}
          </button>
        </div>
      </Transition>
    </div>

    <!-- Terminal Working Directory -->
    <div class="section-title" style="margin-top: 1.5rem;"><FolderOpen size="14" style="margin-right: 5px;"/> 终端工作目录</div>
    <div class="cwd-section">
      <div class="cwd-display">
        <span class="cwd-label">当前目录</span>
        <span class="cwd-path" :title="terminalCwd || '默认沙箱'">{{ terminalCwd || '默认沙箱' }}</span>
      </div>
      <div class="cwd-input-row">
        <input
          v-model="terminalCwd"
          placeholder="输入绝对路径，如 /Users/lhy/Desktop"
          class="cwd-input"
          @keydown.enter="saveTerminalCwd"
        />
        <button
          class="cwd-save-btn"
          :class="{ saved: cwdSaveStatus === 'saved', error: cwdSaveStatus === 'error' }"
          @click="saveTerminalCwd"
          title="保存工作目录"
        >
          {{ cwdSaveStatus === 'saved' ? '✓' : cwdSaveStatus === 'error' ? '✕' : '确定' }}
        </button>
      </div>
      <p class="cwd-hint">此设置优先级高于 AI 的路径选择</p>
    </div>

    </div><!-- end sidebar-scroll -->

    <!-- Bottom Actions -->
    <div class="sidebar-bottom">

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

/* Conversations */
.conv-new-btn {
  margin-left: auto;
  background: transparent;
  border: none;
  color: var(--accent-color);
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  transition: background 0.15s;
}
.conv-new-btn:hover { background: rgba(99,102,241,0.12); }

.conv-list {
  display: flex;
  flex-direction: column;
  gap: 3px;
  max-height: 200px;
  overflow-y: auto;
  margin-bottom: 0.5rem;
}

.conv-item {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 10px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
  color: var(--text-secondary);
  border: 1px solid transparent;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
  min-height: 32px;
}
.conv-item:hover { background: var(--input-bg); color: var(--text-primary); }
.conv-item.active {
  background: rgba(99,102,241,0.12);
  border-color: rgba(99,102,241,0.35);
  color: var(--accent-color);
}

.conv-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.conv-count {
  font-size: 10px;
  color: var(--text-secondary);
  opacity: 0.6;
  flex-shrink: 0;
}

.conv-action-btn {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  width: 18px;
  height: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 3px;
  font-size: 13px;
  flex-shrink: 0;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s;
}
.conv-item:hover .conv-action-btn { opacity: 1; }
.conv-item.active .conv-action-btn { opacity: 0.7; }
.conv-action-btn.del:hover { background: rgba(239,68,68,0.15); color: #f87171; }
.conv-action-btn:not(.del):hover { background: rgba(99,102,241,0.15); color: var(--accent-color); }

.conv-rename-input {
  flex: 1;
  background: var(--input-bg);
  border: 1px solid var(--accent-color);
  color: var(--text-primary);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 12px;
  outline: none;
  min-width: 0;
}

/* Terminal CWD Section */
.cwd-section {
  background: var(--msg-assistant-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.cwd-display {
  display: flex;
  align-items: center;
  gap: 6px;
}

.cwd-label {
  font-size: 11px;
  color: var(--text-secondary);
  white-space: nowrap;
  flex-shrink: 0;
}

.cwd-path {
  font-size: 11px;
  color: var(--accent-color);
  font-family: 'Fira Code', monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.cwd-input-row {
  display: flex;
  gap: 6px;
}

.cwd-input {
  flex: 1;
  background: var(--input-bg);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 6px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-family: 'Fira Code', monospace;
  outline: none;
  transition: border-color 0.2s;
  min-width: 0;
}

.cwd-input:focus {
  border-color: var(--accent-color);
}

.cwd-save-btn {
  background: var(--accent-color);
  border: none;
  color: white;
  padding: 6px 10px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.2s, transform 0.1s;
  flex-shrink: 0;
}

.cwd-save-btn:hover {
  background: var(--accent-hover);
}

.cwd-save-btn:active {
  transform: scale(0.95);
}

.cwd-save-btn.saved {
  background: #10b981;
}

.cwd-save-btn.error {
  background: #ef4444;
}

.cwd-hint {
  font-size: 10px;
  color: var(--text-secondary);
  opacity: 0.7;
  margin: 0;
}

/* Skills list */
.skills-scroll {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

/* Sidebar scroll layout */
.sidebar-scroll {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 2px;
  min-height: 0;
}

.sidebar-scroll::-webkit-scrollbar {
  width: 4px;
}
.sidebar-scroll::-webkit-scrollbar-track {
  background: transparent;
}
.sidebar-scroll::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 2px;
}
.sidebar-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--text-secondary);
}

.sidebar-bottom {
  display: flex;
  gap: 10px;
  flex-direction: column;
  padding-top: 1rem;
  flex-shrink: 0;
}

/* Token Usage */
.token-body { padding: 10px 12px 12px; }

.token-summary {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 10px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border-color);
}
.token-summary-item { display: flex; flex-direction: column; align-items: center; flex: 1; }
.token-summary-label { font-size: 10px; color: var(--text-secondary); opacity: 0.7; }
.token-summary-val { font-size: 13px; font-weight: 700; color: var(--accent-color); font-family: 'Fira Code', monospace; }
.token-summary-sep { color: var(--border-color); font-size: 16px; }

.token-breakdown { display: flex; flex-direction: column; gap: 6px; }
.token-row {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 6px 8px;
  border-radius: 6px;
  background: var(--input-bg);
}
.token-row-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.token-type-badge { font-size: 11px; font-weight: 600; color: var(--text-primary); }
.token-row-calls { font-size: 10px; color: var(--text-secondary); opacity: 0.5; }
.token-row-detail {
  display: flex;
  align-items: center;
  gap: 5px;
  font-family: 'Fira Code', monospace;
  font-size: 11px;
}
.token-detail-item { }
.token-detail-item.prompt { color: #60a5fa; }
.token-detail-item.completion { color: #34d399; }
.token-detail-sep { color: var(--text-secondary); opacity: 0.4; font-size: 10px; }
.token-detail-total { font-size: 12px; font-weight: 700; color: var(--accent-color); margin-left: 2px; }

/* Collapse transition */
.collapse-enter-active,
.collapse-leave-active {
  transition: max-height 0.25s ease, opacity 0.2s ease;
  max-height: 600px;
  overflow: hidden;
}
.collapse-enter-from,
.collapse-leave-to {
  max-height: 0;
  opacity: 0;
}
</style>
