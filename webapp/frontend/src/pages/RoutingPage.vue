<script setup>
import { inject, onMounted, ref } from 'vue'
import { connectivityRecommendation, connectivitySummary, loadConnectivityMap } from '../lib/connectivity'

const models = inject('models')
const currentModel = inject('currentModel')
const routingConfig = inject('routingConfig')
const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))
const appActions = inject('appActions')
const connectivityMap = ref({})

const connectivityFor = (name) => {
  return models.value?.find((model) => model.displayName === name) ? name : ''
}

const connectivityStatus = (name) => {
  const item = connectivityMap.value[name]
  if (!name) return '未设置'
  return connectivitySummary(item)
}

const loadConnectivity = async () => {
  try {
    connectivityMap.value = await loadConnectivityMap(api)
  } catch (err) {
    connectivityMap.value = {}
    notify(`加载路由连通性失败: ${err.message}`, 'error')
  }
}

const saveRouting = async () => {
  try {
    await api.post('/api/routing', routingConfig.value)
    await appActions.refreshGlobalData()
    await loadConnectivity()
    notify('路由配置已保存', 'success')
  } catch (err) {
    notify(`保存失败: ${err.message}`, 'error')
  }
}

onMounted(loadConnectivity)
</script>

<template>
  <section class="panel-page routing-page">
    <div class="page-header">
      <div>
        <h2>路由策略</h2>
        <p>配置路由开关、分类器模型、摘要模型和 tier 映射。</p>
      </div>
      <div class="inline-actions">
        <button class="btn-secondary" @click="loadConnectivity">刷新连通性</button>
        <button class="btn-primary" @click="saveRouting">保存</button>
      </div>
    </div>

    <div class="card">
      <div class="section-headline">
        <h3>当前概览</h3>
      </div>
      <div class="meta-grid compact">
        <div>
          <span class="muted">路由状态</span>
          <strong>{{ routingConfig.enabled ? '已启用' : '未启用' }}</strong>
        </div>
        <div>
          <span class="muted">分类器模型</span>
          <strong>{{ routingConfig.router_model || '未设置' }} / {{ connectivityStatus(routingConfig.router_model) }}</strong>
        </div>
        <div>
          <span class="muted">摘要模型</span>
          <strong>{{ routingConfig.summary_model || '未设置' }} / {{ connectivityStatus(routingConfig.summary_model) }}</strong>
        </div>
        <div>
          <span class="muted">默认模型</span>
          <strong>{{ currentModel || '未设置' }}</strong>
        </div>
        <div>
          <span class="muted">Easy / Medium / Hard</span>
          <strong>{{ routingConfig.tiers.easy || '-' }} / {{ routingConfig.tiers.medium || '-' }} / {{ routingConfig.tiers.hard || '-' }}</strong>
        </div>
        <div>
          <span class="muted">绑定完整性</span>
          <strong>
            {{ connectivityFor(routingConfig.router_model) ? 'Router 已绑定' : 'Router 未绑定' }} /
            {{ connectivityFor(routingConfig.summary_model) ? 'Summary 已绑定' : 'Summary 未绑定' }}
          </strong>
        </div>
        <div>
          <span class="muted">Tier 连通性</span>
          <strong>
            E: {{ connectivityStatus(routingConfig.tiers.easy) }} /
            M: {{ connectivityStatus(routingConfig.tiers.medium) }} /
            H: {{ connectivityStatus(routingConfig.tiers.hard) }}
          </strong>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="section-headline">
        <h3>当前建议</h3>
      </div>
      <div class="stack-gap">
        <p v-if="connectivityRecommendation(connectivityMap[routingConfig.router_model])" class="muted">
          Router：{{ connectivityRecommendation(connectivityMap[routingConfig.router_model]) }}
        </p>
        <p v-if="connectivityRecommendation(connectivityMap[routingConfig.summary_model])" class="muted">
          Summary：{{ connectivityRecommendation(connectivityMap[routingConfig.summary_model]) }}
        </p>
        <p v-if="connectivityRecommendation(connectivityMap[routingConfig.tiers.easy])" class="muted">
          Easy：{{ connectivityRecommendation(connectivityMap[routingConfig.tiers.easy]) }}
        </p>
        <p v-if="connectivityRecommendation(connectivityMap[routingConfig.tiers.medium])" class="muted">
          Medium：{{ connectivityRecommendation(connectivityMap[routingConfig.tiers.medium]) }}
        </p>
        <p v-if="connectivityRecommendation(connectivityMap[routingConfig.tiers.hard])" class="muted">
          Hard：{{ connectivityRecommendation(connectivityMap[routingConfig.tiers.hard]) }}
        </p>
      </div>
    </div>

    <div class="card">
      <label class="toggle-line">
        <span>启用路由</span>
        <input v-model="routingConfig.enabled" type="checkbox" />
      </label>
      <div class="form-grid two-col">
        <label class="field">
          <span>分类器模型</span>
          <select v-model="routingConfig.router_model">
            <option value="">未设置</option>
            <option v-for="model in models" :key="model.displayName" :value="model.displayName">{{ model.displayName }}</option>
          </select>
        </label>
        <label class="field">
          <span>摘要压缩模型</span>
          <select v-model="routingConfig.summary_model">
            <option value="">未设置</option>
            <option v-for="model in models" :key="`summary-${model.displayName}`" :value="model.displayName">{{ model.displayName }}</option>
          </select>
        </label>
      </div>
    </div>

    <div class="card">
      <div class="section-headline">
        <h3>Tier 模型</h3>
      </div>
      <div class="form-grid three-col">
        <label class="field">
          <span>Easy</span>
          <select v-model="routingConfig.tiers.easy">
            <option value="">未设置</option>
            <option v-for="model in models" :key="`easy-${model.displayName}`" :value="model.displayName">{{ model.displayName }}</option>
          </select>
        </label>
        <label class="field">
          <span>Medium</span>
          <select v-model="routingConfig.tiers.medium">
            <option value="">未设置</option>
            <option v-for="model in models" :key="`medium-${model.displayName}`" :value="model.displayName">{{ model.displayName }}</option>
          </select>
        </label>
        <label class="field">
          <span>Hard</span>
          <select v-model="routingConfig.tiers.hard">
            <option value="">未设置</option>
            <option v-for="model in models" :key="`hard-${model.displayName}`" :value="model.displayName">{{ model.displayName }}</option>
          </select>
        </label>
      </div>
    </div>
  </section>
</template>

<style scoped>
.routing-page .card {
  padding: 24px;
}

.routing-page .field span,
.routing-page .toggle-line {
  font-size: var(--font-sm);
}

.routing-page .field select {
  min-height: 54px;
  font-size: var(--font-md);
}

.routing-page .meta-grid strong {
  white-space: normal;
}

@media (min-width: 961px) {
  .routing-page .form-grid.two-col {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .routing-page .form-grid.three-col {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
</style>
