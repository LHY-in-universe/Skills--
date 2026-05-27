<script setup>
import { inject, onMounted, ref } from 'vue'

const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))

const tokenStats = ref(null)
const doctorReport = ref(null)
const securityReport = ref(null)
const authProfiles = ref(null)
const runtimeHealth = ref(null)
const modelConnectivity = ref(null)
const recentFailover = ref([])
const observabilitySummary = ref(null)
const observabilityEvents = ref([])

const redactSecrets = (value) => {
  if (Array.isArray(value)) return value.map(redactSecrets)
  if (!value || typeof value !== 'object') return value
  return Object.fromEntries(
    Object.entries(value).map(([key, nested]) => {
      const lowered = key.toLowerCase()
      if (lowered.includes('key') || lowered.includes('secret') || lowered.includes('token')) {
        return [key, nested ? '***' : nested]
      }
      return [key, redactSecrets(nested)]
    }),
  )
}

const loadDiagnostics = async () => {
  try {
    const [tokens, doctor, security, auth, health, connectivity, failover, summary, events] = await Promise.all([
      api.get('/api/token-usage'),
      api.get('/api/doctor'),
      api.get('/api/security-audit'),
      api.get('/api/auth-profiles'),
      api.get('/api/runtime-health'),
      api.get('/api/model-connectivity'),
      api.get('/api/failover/recent?limit=8'),
      api.get('/api/observability/summary'),
      api.get('/api/observability/events?limit=20'),
    ])
    tokenStats.value = tokens
    doctorReport.value = doctor
    securityReport.value = security
    authProfiles.value = auth
    runtimeHealth.value = health
    modelConnectivity.value = connectivity
    recentFailover.value = Array.isArray(failover?.items) ? failover.items : []
    observabilitySummary.value = summary
    observabilityEvents.value = Array.isArray(events?.items) ? events.items : []
  } catch (err) {
    notify(`加载诊断失败: ${err.message}`, 'error')
  }
}

const runDoctorFix = async (dryRun) => {
  try {
    const res = await api.post('/api/doctor/fix', { dry_run: dryRun })
    notify(`Doctor ${dryRun ? '预检' : '修复'}完成`, 'success')
    if (!dryRun) await loadDiagnostics()
    return res
  } catch (err) {
    notify(`Doctor 执行失败: ${err.message}`, 'error')
    return null
  }
}

onMounted(loadDiagnostics)
</script>

<template>
  <section class="panel-page">
    <div class="page-header">
      <div>
        <h2>诊断中心</h2>
        <p>查看 token、健康状态、授权概况、failover 和可观测性事件。</p>
      </div>
      <div class="inline-actions">
        <button class="btn-secondary" @click="runDoctorFix(true)">Doctor 预检</button>
        <button class="btn-primary" @click="runDoctorFix(false)">Doctor 修复</button>
      </div>
    </div>

    <div class="card-grid">
      <div class="card">
        <div class="section-headline"><h3>Token</h3></div>
        <div class="meta-grid compact">
          <div><span class="muted">总量</span><strong>{{ tokenStats?.global?.total || 0 }}</strong></div>
          <div><span class="muted">Prompt</span><strong>{{ tokenStats?.global?.prompt || 0 }}</strong></div>
          <div><span class="muted">Completion</span><strong>{{ tokenStats?.global?.completion || 0 }}</strong></div>
          <div><span class="muted">Failover</span><strong>{{ tokenStats?.global?.failover?.success || 0 }}/{{ tokenStats?.global?.failover?.count || 0 }}</strong></div>
        </div>
      </div>
      <div class="card">
        <div class="section-headline"><h3>运行时健康</h3></div>
        <pre class="compact-pre">{{ JSON.stringify(runtimeHealth, null, 2) }}</pre>
      </div>
      <div class="card">
        <div class="section-headline"><h3>鉴权画像</h3></div>
        <pre class="compact-pre">{{ JSON.stringify(redactSecrets(authProfiles), null, 2) }}</pre>
      </div>
      <div class="card">
        <div class="section-headline"><h3>安全审计</h3></div>
        <pre class="compact-pre">{{ JSON.stringify(securityReport, null, 2) }}</pre>
      </div>
    </div>

    <div class="card">
      <div class="section-headline"><h3>模型连通性</h3></div>
      <pre class="compact-pre">{{ JSON.stringify(modelConnectivity, null, 2) }}</pre>
    </div>

    <div class="card">
      <div class="section-headline"><h3>Doctor 报告</h3></div>
      <pre class="compact-pre">{{ JSON.stringify(doctorReport, null, 2) }}</pre>
    </div>

    <div class="card">
      <div class="section-headline"><h3>最近 Failover</h3></div>
      <div class="stack-gap">
        <div v-for="(item, idx) in recentFailover" :key="idx" class="subcard">
          <pre class="compact-pre">{{ JSON.stringify(item, null, 2) }}</pre>
        </div>
        <p v-if="recentFailover.length === 0" class="muted">暂无 failover 记录。</p>
      </div>
    </div>

    <div class="card">
      <div class="section-headline"><h3>可观测性概览</h3></div>
      <pre class="compact-pre">{{ JSON.stringify(observabilitySummary, null, 2) }}</pre>
    </div>

    <div class="card">
      <div class="section-headline"><h3>最近事件</h3></div>
      <div class="stack-gap">
        <div v-for="(item, idx) in observabilityEvents" :key="idx" class="subcard">
          <pre class="compact-pre">{{ JSON.stringify(item, null, 2) }}</pre>
        </div>
        <p v-if="observabilityEvents.length === 0" class="muted">暂无事件。</p>
      </div>
    </div>
  </section>
</template>
