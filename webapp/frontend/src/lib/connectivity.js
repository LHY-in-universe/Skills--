export const buildConnectivityMap = (payload) => {
  const items = Array.isArray(payload?.items) ? payload.items : []
  return Object.fromEntries(items.map((item) => [item.model_name, item]))
}

export const loadConnectivityMap = async (api) => {
  const data = await api.get('/api/model-connectivity')
  return buildConnectivityMap(data)
}

export const connectivitySummary = (item) => {
  if (!item) return '未检查'
  if (item.ok) return '连通 OK'
  const code = item.status ? ` (${item.status})` : ''
  return `连通 FAIL${code}`
}

export const connectivityRecommendation = (item) => {
  if (!item) return ''
  return item.recommendation || item.diagnosis || ''
}
