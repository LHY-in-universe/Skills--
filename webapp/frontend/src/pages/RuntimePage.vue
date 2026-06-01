<script setup>
import { inject, onMounted, ref } from 'vue'

const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))
const appActions = inject('appActions')

const embeddingPolicy = ref('disable_when_non_sf')
const selfCorrectionEnabled = ref(true)
const selfCorrectionMaxRetries = ref(1)
const loopGuardEnabled = ref(true)
const plannerEnabled = ref(false)
const plannerMaxSteps = ref(3)
const backendHost = ref('127.0.0.1')
const backendPort = ref(18000)
const visionModelDefault = ref('')
const visionModelSiliconflow = ref('')
const visionModelDeepseek = ref('')
const visionModelKimi = ref('')

const loadRuntimeSettings = async () => {
  try {
    const data = await api.get('/api/runtime-settings')
    embeddingPolicy.value = data.embedding_policy || 'disable_when_non_sf'
    selfCorrectionEnabled.value = data.self_correction_enabled !== false
    selfCorrectionMaxRetries.value = Number.isFinite(data.self_correction_max_retries) ? data.self_correction_max_retries : 1
    loopGuardEnabled.value = data.loop_guard_enabled !== false
    plannerEnabled.value = data.planner_enabled === true
    plannerMaxSteps.value = Number.isFinite(data.planner_max_steps) ? data.planner_max_steps : 3
    backendHost.value = data.backend_host || '127.0.0.1'
    backendPort.value = Number.isFinite(data.backend_port) ? data.backend_port : 18000
    const vm = data.vision_models || {}
    visionModelDefault.value = vm.default || ''
    visionModelSiliconflow.value = vm.siliconflow || ''
    visionModelDeepseek.value = vm.deepseek || ''
    visionModelKimi.value = vm.kimi || ''
  } catch (err) {
    notify(`加载运行时设置失败: ${err.message}`, 'error')
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
      backend_host: backendHost.value.trim() || '127.0.0.1',
      backend_port: Math.max(1, Math.min(65535, Number(backendPort.value) || 18000)),
      vision_models: {
        default: visionModelDefault.value.trim(),
        siliconflow: visionModelSiliconflow.value.trim(),
        deepseek: visionModelDeepseek.value.trim(),
        kimi: visionModelKimi.value.trim(),
      },
    })
    await appActions.refreshGlobalData()
    notify('运行时设置已保存', 'success')
  } catch (err) {
    notify(`保存失败: ${err.message}`, 'error')
  }
}

onMounted(loadRuntimeSettings)
</script>

<template>
  <section class="panel-page">
    <div class="page-header">
      <div>
        <h2>运行时策略</h2>
        <p>调整 embedding、自我修正、planner、后端监听地址和视觉模型默认值。</p>
      </div>
      <button class="btn-primary" @click="saveRuntimeSettings">保存</button>
    </div>

    <div class="card">
      <div class="form-grid two-col">
        <label class="field">
          <span>Embedding 策略</span>
          <select v-model="embeddingPolicy">
            <option value="disable_when_non_sf">非 SiliconFlow 禁用</option>
            <option value="always_on">始终启用</option>
            <option value="always_off">始终关闭</option>
          </select>
        </label>
        <label class="field">
          <span>自我修正重试次数</span>
          <input v-model.number="selfCorrectionMaxRetries" type="number" min="0" max="3" />
        </label>
        <label class="toggle-line">
          <span>启用自我修正</span>
          <input v-model="selfCorrectionEnabled" type="checkbox" />
        </label>
        <label class="toggle-line">
          <span>启用循环保护</span>
          <input v-model="loopGuardEnabled" type="checkbox" />
        </label>
        <label class="toggle-line">
          <span>启用 Planner</span>
          <input v-model="plannerEnabled" type="checkbox" />
        </label>
        <label class="field">
          <span>Planner 最大步数</span>
          <input v-model.number="plannerMaxSteps" type="number" min="1" max="8" />
        </label>
        <label class="field">
          <span>后端监听 Host</span>
          <input v-model="backendHost" placeholder="127.0.0.1 或 0.0.0.0" />
        </label>
        <label class="field">
          <span>后端监听 Port</span>
          <input v-model.number="backendPort" type="number" min="1" max="65535" />
        </label>
      </div>
    </div>

    <div class="card">
      <div class="section-headline">
        <h3>视觉模型默认值</h3>
      </div>
      <div class="form-grid two-col">
        <label class="field">
          <span>默认</span>
          <input v-model="visionModelDefault" />
        </label>
        <label class="field">
          <span>SiliconFlow</span>
          <input v-model="visionModelSiliconflow" />
        </label>
        <label class="field">
          <span>DeepSeek</span>
          <input v-model="visionModelDeepseek" />
        </label>
        <label class="field">
          <span>Kimi</span>
          <input v-model="visionModelKimi" />
        </label>
      </div>
    </div>
  </section>
</template>
