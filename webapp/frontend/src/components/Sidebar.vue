<script setup>
import { inject, ref, computed, onMounted, watch } from 'vue'
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
const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))

const normalizeApiKey = (v) => (v || '').replace(/\s+/g, '')

const customApiUrl = ref('')
const customApiKey = ref('')
const customModel = ref('')

const isManagingModels = ref(false)
const isSkillsOpen = ref(true)       // expanded by default
const newModelName = ref('')
const newModelId = ref('')
const newModelProvider = ref('siliconflow')
const newModelUrl = ref('')
const providerCatalog = ref([])
const providerMetaById = computed(() =>
  Object.fromEntries((providerCatalog.value || []).map(p => [p.id, p]))
)
const selectedProviderMeta = computed(() => providerMetaById.value[newModelProvider.value] || null)

// Conversation management
const editingConvId = ref(null)
const editingConvName = ref('')

const startRename = (conv) => {
  editingConvId.value = conv.id
  editingConvName.value = conv.name
}

const confirmRename = async (convId) => {
  if (!editingConvName.value.trim()) return cancelRename()
  await api.patch(`/api/conversations/${convId}`, { name: editingConvName.value })
  editingConvId.value = null
  emit('refresh-data')
}

const cancelRename = () => { editingConvId.value = null }

const deleteConversation = async (convId) => {
  await api.del(`/api/conversations/${convId}`)
  emit('refresh-data')
}

const toggleSkill = async (skill) => {
  try {
    await api.post('/api/skills/toggle', { name: skill.name, enabled: !skill.enabled })
    emit('refresh-data')
  } catch (err) {
    console.error('Failed to toggle skill:', err)
  }
}

const updateSkillConfig = async (skill) => {
  try {
    await api.patch(`/api/skills/${encodeURIComponent(skill.name)}`, {
      enabled: skill.enabled,
      api_key_ref: skill?.config?.api_key_ref || null,
      env: skill?.config?.env || {},
    })
    notify(`${skill.name} 配置已保存`, 'success')
    emit('refresh-data')
  } catch (err) {
    notify(`Skill 保存失败: ${err.message}`, 'error')
  }
}

const updateConfig = async () => {
  try {
    const cleanedApiKey = normalizeApiKey(customApiKey.value)
    await api.post('/api/config', {
      api_url: customApiUrl.value || undefined,
      api_key: cleanedApiKey || undefined,
      model: customModel.value || undefined
    })
    notify('Configuration updated successfully!', 'success')
    emit('refresh-data')
  } catch (err) {
    notify('Error updating configuration: ' + err.message, 'error')
  }
}

const onModelChange = async (e) => {
  const val = e.target.value
  await api.post('/api/chat/abort').catch(() => {})
  await api.post('/api/config', { model: val })
  currentModel.value = val
  const selected = (models.value || []).find(m => m.displayName === val)
  apiConfig.value = {
    ...(apiConfig.value || {}),
    current_model: val,
    effective_model_id: selected?.apiId || (apiConfig.value?.effective_model_id || ''),
    effective_provider: getProviderName(selected || {}) || (apiConfig.value?.effective_provider || ''),
    effective_api_url: selected?.apiUrl || (apiConfig.value?.effective_api_url || '')
  }
  emit('refresh-data')
}

const addModel = async () => {
  if (!newModelName.value || !newModelId.value) return
  try {
    await api.post('/api/models', {
      name: newModelName.value,
      model_id: newModelId.value,
      provider: newModelProvider.value || 'siliconflow',
      api_url: newModelUrl.value || undefined,
    })
    newModelName.value = ''
    newModelId.value = ''
    newModelProvider.value = 'siliconflow'
    newModelUrl.value = ''
    emit('refresh-data')
  } catch (err) {
    console.error('Failed to add model:', err)
  }
}

const loadProviderCatalog = async () => {
  try {
    const data = await api.get('/api/providers/catalog')
    providerCatalog.value = Array.isArray(data) ? data : []
  } catch (err) {
    console.error('Failed to load provider catalog:', err)
    providerCatalog.value = []
  }
}

watch(newModelProvider, (nextProvider) => {
  const meta = providerMetaById.value[nextProvider]
  if (!meta) return
  if (!newModelUrl.value.trim()) {
    newModelUrl.value = meta.default_api_url || ''
  }
})

const updateModel = async (m) => {
  try {
    await api.patch(`/api/models/${m.displayName}`, {
      model_id: m.apiId,
      api_url: m.apiUrl || undefined,
      provider: m.provider || undefined
    })
    notify(`${m.displayName} 配置已保存`, 'success')
    emit('refresh-data')
  } catch (err) {
    console.error('Failed to update model:', err)
  }
}

const deleteModel = async (name) => {
  if (!confirm(`Are you sure you want to delete ${name}?`)) return
  try {
    await api.del(`/api/models/${name}`)
    emit('refresh-data')
  } catch (err) {
    console.error('Failed to delete model:', err)
  }
}

const getProviderFromUrl = (url) => {
  if (!url) return null
  try {
    const host = url.toLowerCase()
    if (host.includes('deepseek')) return 'DeepSeek'
    if (host.includes('siliconflow')) return 'SiliconFlow'
    if (host.includes('moonshot')) return 'Moonshot'
    if (host.includes('openai')) return 'OpenAI'
    if (host.includes('localhost') || host.includes('127.0.0.1')) return 'Local / Ollama'
    const match = url.match(/https?:\/\/([^/:]+)/i)
    if (match && match[1]) {
      const parts = match[1].split('.')
      return parts.length > 1 ? parts[parts.length - 2] : parts[0]
    }
    return 'Custom Provider'
  } catch { return 'Custom Provider' }
}

const getProviderFromModelId = (apiId) => {
  if (!apiId) return 'SiliconFlow'
  const id = apiId.toLowerCase()
  if (id.includes('deepseek')) return 'DeepSeek'
  if (id.includes('qwen') || id.includes('glm') || id.includes('internlm') || id.includes('pro/') || id.includes('zai-org')) return 'SiliconFlow'
  return 'SiliconFlow'
}

const getProviderName = (m) => {
  if (m.provider) {
    if (m.provider === 'deepseek') return 'DeepSeek'
    if (m.provider === 'kimi') return 'KimiCoding'
    if (m.provider === 'kimi-coding') return 'KimiCoding'
    return 'SiliconFlow'
  }
  // Priority 1: explicit URL override
  if (m.apiUrl) return getProviderFromUrl(m.apiUrl)
  // Priority 2: smart detection from model API ID
  return getProviderFromModelId(m.apiId)
}

const groupedModels = computed(() => {
  const groups = {}
  models.value.forEach(m => {
    const provider = getProviderName(m)
    if (!groups[provider]) groups[provider] = []
    groups[provider].push(m)
  })
  return groups
})

const activeRuntimeText = computed(() => {
  const provider =
    getProviderFromUrl(apiConfig.value?.effective_api_url || '') ||
    apiConfig.value?.effective_provider ||
    '-'
  const modelId = apiConfig.value?.effective_model_id || '-'
  return `${provider} · ${modelId}`
})

// Terminal working directory
const terminalCwd = ref('')
const cwdSaveStatus = ref('')  // '', 'saved', 'error'

const loadTerminalCwd = async () => {
  try {
    const res = await api.get('/api/terminal/cwd')
    terminalCwd.value = res.cwd || ''
  } catch (err) {
    console.error('Failed to load terminal cwd:', err)
  }
}

const saveTerminalCwd = async () => {
  try {
    const data = await api.post('/api/terminal/cwd', { cwd: terminalCwd.value })
    terminalCwd.value = data.cwd || ''
    cwdSaveStatus.value = 'saved'
    setTimeout(() => { cwdSaveStatus.value = '' }, 2000)
  } catch (err) {
    cwdSaveStatus.value = 'error'
    notify(err.message || '目录无效', 'error')
    setTimeout(() => { cwdSaveStatus.value = '' }, 2000)
  }
}

// Routing config
const isRoutingOpen = ref(false)
const routingSaveStatus = ref('')
const isLarkOpen = ref(false)
const isEmbeddingPolicyOpen = ref(false)
const larkAppId = ref('')
const larkAppSecret = ref('')
const larkHasSecret = ref(false)
const larkSaveStatus = ref('')
const embeddingPolicy = ref('disable_when_non_sf')
const embeddingPolicySaveStatus = ref('')
const selfCorrectionEnabled = ref(true)
const selfCorrectionMaxRetries = ref(1)
const loopGuardEnabled = ref(true)
const plannerEnabled = ref(false)
const plannerMaxSteps = ref(3)
const visionModelDefault = ref('')
const visionModelSiliconflow = ref('')
const visionModelDeepseek = ref('')
const visionModelKimi = ref('')
const isDiagnosticsOpen = ref(false)
const doctorReport = ref(null)
const securityReport = ref(null)
const authProfiles = ref(null)
const runtimeHealth = ref(null)
const recentFailover = ref([])
const observabilitySummary = ref(null)
const observabilityEvents = ref([])

const saveRouting = async () => {
  try {
    await api.post('/api/routing', routingConfig.value)
    routingSaveStatus.value = 'saved'
    setTimeout(() => { routingSaveStatus.value = '' }, 2000)
  } catch {
    routingSaveStatus.value = 'error'
    setTimeout(() => { routingSaveStatus.value = '' }, 2000)
  }
}

const loadLarkConfig = async () => {
  try {
    const data = await api.get('/api/lark/config')
    larkAppId.value = data.app_id || ''
    larkHasSecret.value = !!data.has_app_secret
  } catch (err) {
    console.error('Failed to load Lark config:', err)
  }
}

const loadRuntimeSettings = async () => {
  try {
    const data = await api.get('/api/runtime-settings')
    embeddingPolicy.value = data.embedding_policy || 'disable_when_non_sf'
    selfCorrectionEnabled.value = data.self_correction_enabled !== false
    selfCorrectionMaxRetries.value = Number.isFinite(data.self_correction_max_retries)
      ? data.self_correction_max_retries
      : 1
    loopGuardEnabled.value = data.loop_guard_enabled !== false
    plannerEnabled.value = data.planner_enabled === true
    plannerMaxSteps.value = Number.isFinite(data.planner_max_steps)
      ? data.planner_max_steps
      : 3
    const vm = data.vision_models || {}
    visionModelDefault.value = vm.default || ''
    visionModelSiliconflow.value = vm.siliconflow || ''
    visionModelDeepseek.value = vm.deepseek || ''
    visionModelKimi.value = vm.kimi || ''
  } catch (err) {
    console.error('Failed to load runtime settings:', err)
  }
}

const saveRuntimeSettings = async () => {
  try {
    await api.post('/api/runtime-settings', {
      embedding_policy: embeddingPolicy.value,
      self_correction_enabled: selfCorrectionEnabled.value,
      self_correction_max_retries: Math.max(0, Math.min(3, Number(selfCorrectionMaxRetries.value) || 0)),
      loop_guard_enabled: loopGuardEnabled.value,
      planner_enabled: plannerEnabled.value,
      planner_max_steps: Math.max(1, Math.min(8, Number(plannerMaxSteps.value) || 3)),
      vision_models: {
        default: visionModelDefault.value.trim(),
        siliconflow: visionModelSiliconflow.value.trim(),
        deepseek: visionModelDeepseek.value.trim(),
        kimi: visionModelKimi.value.trim(),
      },
    })
    embeddingPolicySaveStatus.value = 'saved'
    setTimeout(() => { embeddingPolicySaveStatus.value = '' }, 2000)
    emit('refresh-data')
  } catch (err) {
    embeddingPolicySaveStatus.value = 'error'
    setTimeout(() => { embeddingPolicySaveStatus.value = '' }, 2000)
  }
}

const saveLarkConfig = async () => {
  if (!larkAppId.value.trim() || !larkAppSecret.value.trim()) {
    notify('请填写 LARK App ID 和 App Secret', 'error')
    return
  }
  try {
    await api.post('/api/lark/config', {
      app_id: larkAppId.value.trim(),
      app_secret: larkAppSecret.value.trim()
    })
    larkSaveStatus.value = 'saved'
    larkHasSecret.value = true
    larkAppSecret.value = ''
    notify('Lark 配置已保存。请重启后端使其生效。', 'success')
    setTimeout(() => { larkSaveStatus.value = '' }, 2000)
  } catch (err) {
    larkSaveStatus.value = 'error'
    setTimeout(() => { larkSaveStatus.value = '' }, 2000)
  }
}

// Token usage stats
const tokenStats = ref(null)
const isTokenOpen = ref(false)

const todayTokens = computed(() => {
  const today = new Date().toISOString().slice(0, 10)
  return tokenStats.value?.daily?.[today]?.total ?? 0
})
const failoverSummary = computed(() => {
  const f = tokenStats.value?.global?.failover
  if (!f) return '0/0'
  return `${f.success || 0}/${f.count || 0}`
})

const TYPE_LABELS = {
  chat: '对话', skill: 'Skill 调用', router: '路由',
  compress: '压缩', embedding_ctx: '向量(上下文)', embedding_mem: '向量(记忆)'
}

const fetchTokenStats = async () => {
  try {
    tokenStats.value = await api.get('/api/token-usage')
  } catch {}
}

const loadDiagnostics = async () => {
  try {
    const [d, s, a, h, f, o, e] = await Promise.all([
      api.get('/api/doctor'),
      api.get('/api/security-audit'),
      api.get('/api/auth-profiles'),
      api.get('/api/runtime-health'),
      api.get('/api/failover/recent?limit=8'),
      api.get('/api/observability/summary'),
      api.get('/api/observability/events?limit=20'),
    ])
    doctorReport.value = d
    securityReport.value = s
    authProfiles.value = a
    runtimeHealth.value = h
    recentFailover.value = Array.isArray(f?.items) ? f.items : []
    observabilitySummary.value = o
    observabilityEvents.value = Array.isArray(e?.items) ? e.items : []
  } catch (err) {
    console.error('Failed to load diagnostics:', err)
  }
}

const runDoctorFix = async () => {
  try {
    const res = await api.post('/api/doctor/fix', { dry_run: false })
    notify(`Doctor 修复完成\napplied: ${(res.applied || []).join(', ') || '-'}\nskipped: ${(res.skipped || []).join(', ') || '-'}`, 'success')
    await loadDiagnostics()
  } catch (err) {
    notify('Doctor 修复失败: ' + err.message, 'error')
  }
}

const previewDoctorFix = async () => {
  try {
    const res = await api.post('/api/doctor/fix', { dry_run: true })
    notify(`Doctor 预检\napplied: ${(res.applied || []).join(', ') || '-'}\nskipped: ${(res.skipped || []).join(', ') || '-'}`, 'success')
  } catch (err) {
    notify('Doctor 预检失败: ' + err.message, 'error')
  }
}

defineExpose({ fetchTokenStats })

onMounted(() => { loadTerminalCwd(); fetchTokenStats(); loadLarkConfig(); loadRuntimeSettings(); loadProviderCatalog(); loadDiagnostics() })
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
        <optgroup v-for="(group, provider) in groupedModels" :key="provider" :label="provider.toUpperCase()">
          <option v-for="m in group" :key="m.displayName" :value="m.displayName">{{ m.displayName }}</option>
        </optgroup>
      </select>
      <div style="margin-top: 6px; font-size: 10px; color: var(--text-secondary); opacity: 0.8;">
        实际路由：{{ activeRuntimeText }}
      </div>
    </div>

    <div v-else class="config-group" style="background: var(--msg-assistant-bg); padding: 12px; border-radius: 8px; border: 1px solid var(--border-color); display: flex; flex-direction: column; gap: 20px;">
      <div v-for="(group, provider) in groupedModels" :key="provider">
        <div style="font-size: 11px; font-weight: 800; color: var(--accent-color); margin-bottom: 10px; display: flex; align-items: center; gap: 6px; letter-spacing: 0.05em;">
          <Globe size="12" /> {{ provider.toUpperCase() }}
        </div>
        <div style="display: flex; flex-direction: column; gap: 8px; padding-left: 8px; border-left: 2px solid rgba(99,102,241,0.2);">
          <div v-for="m in group" :key="m.displayName" style="display: flex; align-items: center; justify-content: space-between; padding: 6px 8px; border-radius: 6px; background: var(--input-bg);">
            <div>
              <div style="font-weight: 700; color: var(--text-primary); font-size: 13px;">{{ m.displayName }}</div>
              <div style="font-size: 10px; color: var(--text-secondary); opacity: 0.7; margin-top: 2px;">{{ m.apiId }}</div>
            </div>
            <button @click="deleteModel(m.displayName)" style="background: transparent; border: none; color: #f87171; cursor: pointer; padding: 0 5px; font-size: 18px;">&times;</button>
          </div>
        </div>
      </div>
      
      <div style="margin-top: 10px; padding-top: 15px; border-top: 2px solid var(--border-color); display: flex; flex-direction: column; gap: 6px;">
        <div style="font-size: 11px; font-weight: 800; color: var(--text-secondary); margin-bottom: 4px;">+ ADD NEW MODEL</div>
        <input v-model="newModelName" placeholder="Display Name (e.g. DeepSeek-R1)" style="background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); padding: 7px; border-radius: 4px; font-size: 11px;" />
        <input v-model="newModelId" placeholder="API ID (e.g. deepseek-ai/DeepSeek-R1)" style="background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); padding: 7px; border-radius: 4px; font-size: 11px;" />
        <select v-model="newModelProvider" style="background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); padding: 7px; border-radius: 4px; font-size: 11px;">
          <option v-for="p in providerCatalog" :key="p.id" :value="p.id">Provider: {{ p.label }}</option>
        </select>
        <input v-model="newModelUrl" placeholder="API URL (仅新增时可选)" style="background: var(--input-bg); border: 1px solid var(--border-color); color: var(--text-primary); padding: 7px; border-radius: 4px; font-size: 11px;" />
        <div v-if="selectedProviderMeta" style="font-size:10px;color:var(--text-secondary);opacity:.8;">
          默认 URL: {{ selectedProviderMeta.default_api_url }}<br />
          所需环境变量: {{ (selectedProviderMeta.required_env_keys || []).join(', ') || '-' }}
        </div>
        <div style="font-size:10px;color:var(--text-secondary);opacity:.75;">API Key 改为仅后端 `.env` 管理。</div>
        <button @click="addModel" style="background: var(--accent-color); border: none; color: white; padding: 8px; border-radius: 4px; font-size: 12px; font-weight: 600; cursor: pointer; margin-top: 3px;">Add Model</button>
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
              <div style="flex:1;min-width:0;">
                <div style="display:flex;align-items:center;justify-content:space-between;gap:8px;">
                  <span style="font-size: 13px;">{{ skill.name }}</span>
                  <label class="switch">
                    <input type="checkbox" :checked="skill.enabled" @change="toggleSkill(skill)">
                    <span class="slider"></span>
                  </label>
                </div>
                <div style="font-size:10px;color:var(--text-secondary);opacity:.8;margin-top:2px;">
                  {{ skill.config?.api_key_ref ? `SecretRef: ${skill.config.api_key_ref.source}:${skill.config.api_key_ref.id || skill.config.api_key_ref.path || '-'}` : '无需额外密钥' }}
                  ·
                  {{ skill.config?.secret_ready === false ? '缺少密钥' : '就绪' }}
                </div>
                <div v-if="skill.config?.api_key_ref" style="display:flex;gap:6px;margin-top:6px;">
                  <input
                    v-model="skill.config.api_key_ref.id"
                    placeholder="ENV 变量名"
                    style="flex:1;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);padding:6px 7px;border-radius:4px;font-size:11px;"
                  />
                  <button
                    @click="updateSkillConfig(skill)"
                    style="background:var(--accent-color);border:none;color:#fff;padding:6px 8px;border-radius:4px;font-size:11px;cursor:pointer;"
                  >
                    保存
                  </button>
                </div>
              </div>
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
            <div class="token-summary-sep">·</div>
            <div class="token-summary-item">
              <span class="token-summary-label">回退</span>
              <span class="token-summary-val">{{ failoverSummary }}</span>
            </div>
          </div>
          <div style="font-size:10px;color:var(--text-secondary);margin-top:6px;">
            错误总数: {{ tokenStats?.global?.errors?.count || 0 }} · 平均延迟: {{ tokenStats?.global?.latency?.avg_ms || 0 }}ms
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
              <option v-for="m in models" :key="m.apiId" :value="m.apiId">{{ m.displayName }}</option>
              <option v-if="routingConfig.router_model && !models.some(m => m.apiId === routingConfig.router_model)" :value="routingConfig.router_model">{{ routingConfig.router_model }}</option>
            </select>
          </div>
          <!-- Summary model -->
          <div style="margin-bottom:10px;">
            <div style="font-size:11px;color:var(--text-secondary);margin-bottom:4px;letter-spacing:0.04em;">摘要压缩模型</div>
            <select v-model="routingConfig.summary_model" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:34px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;">
              <option value="">— 未设置（使用当前主模型） —</option>
              <option v-for="m in models" :key="m.apiId" :value="m.apiId">{{ m.displayName }}</option>
              <option v-if="routingConfig.summary_model && !models.some(m => m.apiId === routingConfig.summary_model)" :value="routingConfig.summary_model">{{ routingConfig.summary_model }}</option>
            </select>
            <div style="font-size:10px;color:var(--text-secondary);opacity:0.6;margin-top:3px;">用于对话历史压缩，推荐小模型</div>
          </div>
          <!-- Tier models -->
          <div style="display:flex;flex-direction:column;gap:7px;margin-bottom:12px;">
            <div v-for="(label, tier) in { easy: '🟢 简单', medium: '🟡 中等', hard: '🔴 困难' }" :key="tier">
              <div style="font-size:11px;color:var(--text-secondary);margin-bottom:3px;">{{ label }}</div>
              <select v-model="routingConfig.tiers[tier]" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:32px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;">
                <option value="">— 未设置（使用默认） —</option>
                <option v-for="m in models" :key="m.apiId" :value="m.apiId">{{ m.displayName }}</option>
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

    <!-- Embedding Policy -->
    <div class="collapsible-section" style="margin-top: 1.5rem;">
      <button class="collapsible-header" @click="isEmbeddingPolicyOpen = !isEmbeddingPolicyOpen">
        <span style="display:flex;align-items:center;gap:6px;">
          <Globe size="13"/> EMBEDDING 策略
        </span>
        <ChevronDown size="14" :style="{ transform: isEmbeddingPolicyOpen ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s' }" />
      </button>
      <Transition name="collapse">
        <div v-if="isEmbeddingPolicyOpen" class="collapsible-body">
          <select v-model="embeddingPolicy" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:34px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;margin-bottom:8px;">
            <option value="always_siliconflow">always_siliconflow</option>
            <option value="disable_when_non_sf">disable_when_non_sf</option>
            <option value="follow_chat_provider">follow_chat_provider</option>
          </select>
          <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:8px;">
            <span style="font-size:12px;color:var(--text-secondary);">自纠错</span>
            <label class="switch">
              <input type="checkbox" v-model="selfCorrectionEnabled">
              <span class="slider"></span>
            </label>
          </div>
          <div style="margin-bottom:8px;">
            <div style="font-size:11px;color:var(--text-secondary);margin-bottom:4px;">自纠错重试次数（0-3）</div>
            <input v-model.number="selfCorrectionMaxRetries" type="number" min="0" max="3" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:32px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;" />
          </div>
          <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:8px;">
            <span style="font-size:12px;color:var(--text-secondary);">循环防护</span>
            <label class="switch">
              <input type="checkbox" v-model="loopGuardEnabled">
              <span class="slider"></span>
            </label>
          </div>
          <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:8px;">
            <span style="font-size:12px;color:var(--text-secondary);">规划器</span>
            <label class="switch">
              <input type="checkbox" v-model="plannerEnabled">
              <span class="slider"></span>
            </label>
          </div>
          <div style="margin-bottom:8px;">
            <div style="font-size:11px;color:var(--text-secondary);margin-bottom:4px;">规划最大步数（1-8）</div>
            <input v-model.number="plannerMaxSteps" type="number" min="1" max="8" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:32px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;" />
          </div>
          <div style="margin-bottom:8px;border-top:1px dashed var(--border-color);padding-top:8px;">
            <div style="font-size:11px;color:var(--text-secondary);margin-bottom:6px;">视觉模型映射（/vision）</div>
            <input v-model="visionModelSiliconflow" placeholder="siliconflow 视觉模型ID（可空）" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:32px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;margin-bottom:6px;" />
            <input v-model="visionModelDeepseek" placeholder="deepseek 视觉模型ID（可空）" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:32px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;margin-bottom:6px;" />
            <input v-model="visionModelKimi" placeholder="kimi 视觉模型ID（可空）" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:32px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;margin-bottom:6px;" />
            <input v-model="visionModelDefault" placeholder="default 视觉模型ID（可空，provider未配置时使用）" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);height:32px;padding:0 8px;font-size:12px;border-radius:6px;outline:none;" />
            <div style="font-size:10px;color:var(--text-secondary);opacity:.75;margin-top:4px;">未配置时回退当前聊天模型。</div>
          </div>
          <button
            @click="saveRuntimeSettings"
            :style="{
              width: '100%',
              background: embeddingPolicySaveStatus === 'saved' ? '#10b981' : embeddingPolicySaveStatus === 'error' ? '#ef4444' : 'var(--accent-color)',
              border: 'none', color: 'white', padding: '7px', borderRadius: '5px',
              cursor: 'pointer', fontSize: '12px', fontWeight: '600', transition: 'background 0.2s'
            }"
          >
            {{ embeddingPolicySaveStatus === 'saved' ? '✓ 已保存' : embeddingPolicySaveStatus === 'error' ? '✕ 保存失败' : '保存策略' }}
          </button>
        </div>
      </Transition>
    </div>

    <!-- Diagnostics -->
    <div class="collapsible-section" style="margin-top: 1.5rem;">
      <button class="collapsible-header" @click="isDiagnosticsOpen = !isDiagnosticsOpen">
        <span style="display:flex;align-items:center;gap:6px;">
          <BarChart2 size="13"/> 诊断中心
          <span v-if="doctorReport?.ok === false || securityReport?.ok === false" style="font-size:10px;background:rgba(239,68,68,0.2);color:#ef4444;padding:1px 6px;border-radius:10px;font-weight:700;">ISSUES</span>
        </span>
        <ChevronDown size="14" :style="{ transform: isDiagnosticsOpen ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s' }" />
      </button>
      <Transition name="collapse">
        <div v-if="isDiagnosticsOpen" class="collapsible-body">
          <button
            @click="loadDiagnostics"
            style="width:100%;background:var(--accent-color);border:none;color:#fff;padding:7px;border-radius:5px;cursor:pointer;font-size:12px;font-weight:600;margin-bottom:8px;"
          >
            刷新诊断
          </button>
          <button
            @click="previewDoctorFix"
            style="width:100%;background:#22c55e;border:none;color:#fff;padding:7px;border-radius:5px;cursor:pointer;font-size:12px;font-weight:600;margin-bottom:8px;"
          >
            Doctor 预检（Dry Run）
          </button>
          <button
            @click="runDoctorFix"
            style="width:100%;background:#0ea5e9;border:none;color:#fff;padding:7px;border-radius:5px;cursor:pointer;font-size:12px;font-weight:600;margin-bottom:8px;"
          >
            执行 Doctor 修复
          </button>
          <div style="font-size:11px;color:var(--text-secondary);margin-bottom:6px;">
            Doctor: {{ doctorReport?.ok === false ? '异常' : '正常' }} · Security: {{ securityReport?.ok === false ? '风险' : '正常' }}
          </div>
          <div v-if="runtimeHealth" style="font-size:10px;color:var(--text-secondary);margin-bottom:8px;">
            Active Conv: {{ runtimeHealth.active_conversation_id || '-' }} · Models: {{ runtimeHealth.models_count ?? 0 }} · Skills: {{ runtimeHealth.enabled_skills ?? 0 }}
          </div>
          <div v-if="observabilitySummary" style="font-size:10px;color:var(--text-secondary);margin-bottom:8px;">
            Failover 成功率: {{ observabilitySummary?.failover?.success_rate ?? 0 }}% · 错误数: {{ observabilitySummary?.errors?.count ?? 0 }}
            <br />
            执行事件(今日): {{ observabilitySummary?.execution_logs?.today_events ?? 0 }} · 执行错误: {{ observabilitySummary?.execution_logs?.today_errors ?? 0 }}
          </div>
          <div v-if="doctorReport?.findings?.length" style="font-size:10px;color:var(--text-secondary);margin-bottom:8px;">
            <div v-for="(f, i) in doctorReport.findings.slice(0, 4)" :key="'d-'+i">[{{ f.severity }}] {{ f.message }}</div>
          </div>
          <div v-if="securityReport?.findings?.length" style="font-size:10px;color:var(--text-secondary);margin-bottom:8px;">
            <div v-for="(f, i) in securityReport.findings.slice(0, 4)" :key="'s-'+i">[{{ f.severity }}] {{ f.message }}</div>
          </div>
          <div style="font-size:10px;color:var(--text-secondary);opacity:.8;">
            Auth Profiles:
            <span v-if="authProfiles">{{ Object.keys(authProfiles).join(', ') || '-' }}</span>
            <span v-else>-</span>
          </div>
          <div v-if="recentFailover.length" style="font-size:10px;color:var(--text-secondary);margin-top:6px;">
            <div v-for="(it, i) in recentFailover.slice(-4)" :key="'fo-'+i">
              [{{ it.type }}] {{ it.from_model || it.from_profile || '-' }} → {{ it.to_model || it.to_profile || '-' }}
            </div>
          </div>
          <div v-if="observabilityEvents.length" style="font-size:10px;color:var(--text-secondary);margin-top:6px;">
            <div style="margin-bottom:4px;opacity:.85;">最近执行事件</div>
            <div v-for="(it, i) in observabilityEvents.slice(-8)" :key="'ev-'+i">
              [{{ it.event_type }}] {{ it.model || it.tool || '-' }} {{ it.error_class ? `(${it.error_class})` : '' }}
            </div>
          </div>
        </div>
      </Transition>
    </div>

    <!-- Lark Integration -->
    <div class="collapsible-section" style="margin-top: 1.5rem;">
      <button class="collapsible-header" @click="isLarkOpen = !isLarkOpen">
        <span style="display:flex;align-items:center;gap:6px;">
          <Globe size="13"/> LARK 接入
          <span v-if="larkAppId && larkHasSecret" style="font-size:10px;background:rgba(16,185,129,0.2);color:#10b981;padding:1px 6px;border-radius:10px;font-weight:700;">READY</span>
        </span>
        <ChevronDown size="14" :style="{ transform: isLarkOpen ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s' }" />
      </button>
      <Transition name="collapse">
        <div v-if="isLarkOpen" class="collapsible-body">
          <input v-model="larkAppId" placeholder="LARK_APP_ID" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);padding:7px;border-radius:4px;font-size:11px;margin-bottom:6px;" />
          <input v-model="larkAppSecret" type="password" placeholder="LARK_APP_SECRET（保存后会清空）" style="width:100%;background:var(--input-bg);border:1px solid var(--border-color);color:var(--text-primary);padding:7px;border-radius:4px;font-size:11px;margin-bottom:8px;" />
          <div style="font-size:10px;color:var(--text-secondary);opacity:.75;margin-bottom:8px;">保存到 `siliconflow/config/.env`，需重启后端生效。</div>
          <button
            @click="saveLarkConfig"
            :style="{
              width: '100%',
              background: larkSaveStatus === 'saved' ? '#10b981' : larkSaveStatus === 'error' ? '#ef4444' : 'var(--accent-color)',
              border: 'none', color: 'white', padding: '7px', borderRadius: '5px',
              cursor: 'pointer', fontSize: '12px', fontWeight: '600', transition: 'background 0.2s'
            }"
          >
            {{ larkSaveStatus === 'saved' ? '✓ 已保存' : larkSaveStatus === 'error' ? '✕ 保存失败' : '保存 Lark 配置' }}
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

      <!-- Clear Conversation -->

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
