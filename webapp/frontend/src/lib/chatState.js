export const createRoutingConfig = () => ({
  enabled: false,
  router_model: '',
  summary_model: '',
  tiers: {
    easy: '',
    medium: '',
    hard: '',
  },
})

export const createLastRouteInfo = () => ({
  tier: '',
  model: '',
})

export const createLiveUsage = () => ({
  prompt: 0,
  completion: 0,
  total: 0,
  call_type: '',
  model: '',
  provider: '',
  cached_read: 0,
  cached_write: 0,
})

export const createStreamMeta = () => ({
  plan: [],
  currentStep: '',
  audit: '',
  auditReason: '',
  failover: [],
})

export const createVoiceRuntime = () => ({
  enabled: false,
  convId: '',
  phase: 'idle',
  source: '',
  queueLength: 0,
  chunksReceived: 0,
  chunksPlayed: 0,
})

export const createPermissionDialog = () => ({
  visible: false,
  toolName: '',
  description: '',
})

export const createNotice = () => ({
  show: false,
  type: 'info',
  text: '',
})

export const mapModelsResponse = (modelsRes) =>
  Object.entries(modelsRes || {}).map(([displayName, config]) => ({
    displayName,
    apiId: config.id || config,
    provider: config.provider || '',
    apiUrl: config.api_url || '',
    enabled: config.enabled !== false,
    capabilities: config.capabilities || {},
    requires: config.requires || [],
  }))

export const mergeHistoryMetadata = (prevMessages, nextMessages) => {
  const buckets = new Map()
  for (const msg of prevMessages || []) {
    const key = `${msg?.role || ''}|${msg?.content || ''}|${JSON.stringify(msg?.tool_calls || [])}`
    const arr = buckets.get(key) || []
    if (msg && (msg._tokens || msg._model)) {
      arr.push({ _tokens: msg._tokens, _model: msg._model })
    }
    buckets.set(key, arr)
  }
  return (nextMessages || []).map((msg) => {
    const key = `${msg?.role || ''}|${msg?.content || ''}|${JSON.stringify(msg?.tool_calls || [])}`
    const arr = buckets.get(key) || []
    const cached = arr.shift()
    buckets.set(key, arr)
    return cached ? { ...msg, ...cached } : msg
  })
}
