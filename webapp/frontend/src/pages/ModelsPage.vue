<script setup>
import { computed, inject, onMounted, ref, watch } from 'vue'
import { getProviderName } from '../lib/modelMeta'

const models = inject('models')
const currentModel = inject('currentModel')
const routingConfig = inject('routingConfig')
const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))
const appActions = inject('appActions')

const providerCatalog = ref([])
const editableModels = ref([])
const connectivityMap = ref({})
const newModelName = ref('')
const newModelId = ref('')
const newModelProvider = ref('siliconflow')
const newModelUrl = ref('')

const providerMetaById = computed(() =>
  Object.fromEntries((providerCatalog.value || []).map((item) => [item.id, item]))
)

const selectedProviderMeta = computed(() => providerMetaById.value[newModelProvider.value] || null)
const activeModelName = computed(() => currentModel.value || '')
const routingRoles = computed(() => ({
  router: routingConfig.value?.router_model || '',
  summary: routingConfig.value?.summary_model || '',
  easy: routingConfig.value?.tiers?.easy || '',
  medium: routingConfig.value?.tiers?.medium || '',
  hard: routingConfig.value?.tiers?.hard || '',
}))

const modelRoleBadges = (displayName) => {
  const roles = []
  if (displayName === activeModelName.value) roles.push('默认')
  if (displayName === routingRoles.value.router) roles.push('Router')
  if (displayName === routingRoles.value.summary) roles.push('Summary')
  if (displayName === routingRoles.value.easy) roles.push('Easy')
  if (displayName === routingRoles.value.medium) roles.push('Medium')
  if (displayName === routingRoles.value.hard) roles.push('Hard')
  return roles
}

const groupedModels = computed(() => {
  const groups = {}
  for (const model of editableModels.value || []) {
    const provider = getProviderName(model)
    if (!groups[provider]) groups[provider] = []
    groups[provider].push(model)
  }
  return Object.entries(groups)
})

watch(models, (nextModels) => {
  editableModels.value = (nextModels || []).map((model) => ({
    ...model,
    capabilities: { ...(model.capabilities || {}) },
    requires: Array.isArray(model.requires) ? [...model.requires] : [],
  }))
}, { immediate: true })

const loadProviderCatalog = async () => {
  try {
    const data = await api.get('/api/providers/catalog')
    providerCatalog.value = Array.isArray(data) ? data : []
  } catch (err) {
    providerCatalog.value = []
    notify(`加载 provider 列表失败: ${err.message}`, 'error')
  }
}

const loadConnectivity = async () => {
  try {
    const data = await api.get('/api/model-connectivity')
    const items = Array.isArray(data?.items) ? data.items : []
    connectivityMap.value = Object.fromEntries(items.map((item) => [item.model_name, item]))
  } catch (err) {
    connectivityMap.value = {}
    notify(`加载模型连通性失败: ${err.message}`, 'error')
  }
}

const switchModel = async (modelName) => {
  try {
    await api.post('/api/chat/abort').catch(() => {})
    await api.post('/api/config', { model: modelName })
    await appActions.refreshGlobalData()
    notify(`已切换模型: ${modelName}`, 'success')
  } catch (err) {
    notify(`切换模型失败: ${err.message}`, 'error')
  }
}

const addModel = async () => {
  if (!newModelName.value.trim() || !newModelId.value.trim()) return
  try {
    await api.post('/api/models', {
      name: newModelName.value.trim(),
      model_id: newModelId.value.trim(),
      provider: newModelProvider.value || 'siliconflow',
      api_url: newModelUrl.value.trim() || undefined,
    })
    newModelName.value = ''
    newModelId.value = ''
    newModelProvider.value = 'siliconflow'
    newModelUrl.value = ''
    await appActions.refreshGlobalData()
    await loadConnectivity()
    notify('模型已添加', 'success')
  } catch (err) {
    notify(`添加模型失败: ${err.message}`, 'error')
  }
}

const updateModel = async (model) => {
  try {
    await api.patch(`/api/models/${encodeURIComponent(model.displayName)}`, {
      model_id: model.apiId,
      api_url: model.apiUrl || undefined,
      provider: model.provider || undefined,
    })
    await appActions.refreshGlobalData()
    await loadConnectivity()
    notify(`${model.displayName} 已保存`, 'success')
  } catch (err) {
    notify(`保存失败: ${err.message}`, 'error')
  }
}

const deleteModel = async (name) => {
  try {
    await api.del(`/api/models/${encodeURIComponent(name)}`)
    await appActions.refreshGlobalData()
    await loadConnectivity()
    notify(`已删除模型: ${name}`, 'success')
  } catch (err) {
    notify(`删除失败: ${err.message}`, 'error')
  }
}

onMounted(async () => {
  await Promise.all([loadProviderCatalog(), loadConnectivity()])
})
</script>

<template>
  <section class="panel-page">
    <div class="page-header">
      <div>
        <h2>模型管理</h2>
        <p>切换当前模型，维护模型列表和 provider 配置。</p>
      </div>
      <button type="button" class="btn-secondary" @click="appActions.refreshGlobalData()">刷新</button>
    </div>

    <div class="card">
      <div class="section-headline">
        <h3>当前绑定</h3>
      </div>
      <div class="meta-grid compact">
        <div><span class="muted">默认模型</span><strong>{{ activeModelName || '未设置' }}</strong></div>
        <div><span class="muted">Router</span><strong>{{ routingRoles.router || '未设置' }}</strong></div>
        <div><span class="muted">Summary</span><strong>{{ routingRoles.summary || '未设置' }}</strong></div>
        <div><span class="muted">Easy / Medium / Hard</span><strong>{{ routingRoles.easy || '-' }} / {{ routingRoles.medium || '-' }} / {{ routingRoles.hard || '-' }}</strong></div>
      </div>
    </div>

    <div class="card">
      <div class="form-grid two-col">
        <label class="field">
          <span>模型显示名</span>
          <input v-model="newModelName" placeholder="例如 Qwen3-VL-2B-Ollama-Q8" />
        </label>
        <label class="field">
          <span>模型 ID</span>
          <input v-model="newModelId" placeholder="例如 qwen3-vl:2b-instruct-q8_0" />
        </label>
        <label class="field">
          <span>Provider</span>
          <select v-model="newModelProvider">
            <option v-for="item in providerCatalog" :key="item.id" :value="item.id">{{ item.label }}</option>
          </select>
        </label>
        <label class="field">
          <span>API URL</span>
          <input v-model="newModelUrl" :placeholder="selectedProviderMeta?.default_api_url || '可选'" />
        </label>
      </div>
      <div class="page-actions">
        <button type="button" class="btn-primary" @click="addModel">新增模型</button>
      </div>
    </div>

    <div v-for="[provider, items] in groupedModels" :key="provider" class="card stack-gap">
      <div class="section-headline">
        <h3>{{ provider }}</h3>
        <span class="muted">{{ items.length }} 个模型</span>
      </div>
      <div v-for="model in items" :key="model.displayName" class="subcard">
        <div class="card-topline">
          <div>
            <strong>{{ model.displayName }}</strong>
            <div class="inline-badges">
              <span v-for="role in modelRoleBadges(model.displayName)" :key="role" class="pill success">{{ role }}</span>
              <span
                v-if="connectivityMap[model.displayName]"
                class="pill"
                :class="connectivityMap[model.displayName].ok ? 'success' : 'danger'"
              >
                {{ connectivityMap[model.displayName].ok ? '连通 OK' : '连通 FAIL' }}
              </span>
            </div>
          </div>
          <div class="inline-actions">
            <button type="button" class="btn-secondary" @click="switchModel(model.displayName)">设为当前</button>
            <button type="button" class="btn-secondary" @click="updateModel(model)">保存</button>
            <button type="button" class="btn-danger" @click="deleteModel(model.displayName)">删除</button>
          </div>
        </div>
        <div class="form-grid two-col">
          <label class="field">
            <span>模型 ID</span>
            <input v-model="model.apiId" />
          </label>
          <label class="field">
            <span>API URL</span>
            <input v-model="model.apiUrl" />
          </label>
          <label class="field">
            <span>Provider</span>
            <input v-model="model.provider" />
          </label>
          <label class="field">
            <span>Capabilities</span>
            <input :value="Object.keys(model.capabilities || {}).join(', ')" disabled />
          </label>
          <label class="field" v-if="connectivityMap[model.displayName]">
            <span>连通性诊断</span>
            <input :value="connectivityMap[model.displayName].diagnosis || '-'" disabled />
          </label>
        </div>
      </div>
    </div>
  </section>
</template>
