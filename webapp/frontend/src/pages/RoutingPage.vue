<script setup>
import { inject } from 'vue'

const models = inject('models')
const routingConfig = inject('routingConfig')
const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))
const appActions = inject('appActions')

const saveRouting = async () => {
  try {
    await api.post('/api/routing', routingConfig.value)
    await appActions.refreshGlobalData()
    notify('路由配置已保存', 'success')
  } catch (err) {
    notify(`保存失败: ${err.message}`, 'error')
  }
}
</script>

<template>
  <section class="panel-page routing-page">
    <div class="page-header">
      <div>
        <h2>路由策略</h2>
        <p>配置路由开关、分类器模型、摘要模型和 tier 映射。</p>
      </div>
      <button class="btn-primary" @click="saveRouting">保存</button>
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
          <strong>{{ routingConfig.router_model || '未设置' }}</strong>
        </div>
        <div>
          <span class="muted">摘要模型</span>
          <strong>{{ routingConfig.summary_model || '未设置' }}</strong>
        </div>
        <div>
          <span class="muted">Easy / Medium / Hard</span>
          <strong>{{ routingConfig.tiers.easy || '-' }} / {{ routingConfig.tiers.medium || '-' }} / {{ routingConfig.tiers.hard || '-' }}</strong>
        </div>
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
