<script setup>
import { computed, inject, onMounted, ref, watch } from 'vue'
import { getProviderName } from '../lib/modelMeta'

const models = inject('models')
const currentModel = inject('currentModel')
const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))
const appActions = inject('appActions')

const providerCatalog = ref([])
const editableModels = ref([])
const newModelName = ref('')
const newModelId = ref('')
const newModelProvider = ref('siliconflow')
const newModelUrl = ref('')

const providerMetaById = computed(() =>
  Object.fromEntries((providerCatalog.value || []).map((item) => [item.id, item]))
)

const selectedProviderMeta = computed(() => providerMetaById.value[newModelProvider.value] || null)
const activeModelName = computed(() => currentModel.value || '')

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
    notify(`${model.displayName} 已保存`, 'success')
  } catch (err) {
    notify(`保存失败: ${err.message}`, 'error')
  }
}

const deleteModel = async (name) => {
  try {
    await api.del(`/api/models/${encodeURIComponent(name)}`)
    await appActions.refreshGlobalData()
    notify(`已删除模型: ${name}`, 'success')
  } catch (err) {
    notify(`删除失败: ${err.message}`, 'error')
  }
}

onMounted(loadProviderCatalog)
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
            <span v-if="model.displayName === activeModelName" class="pill success">当前</span>
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
        </div>
      </div>
    </div>
  </section>
</template>
