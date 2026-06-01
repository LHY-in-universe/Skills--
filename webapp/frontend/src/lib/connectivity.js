export const buildConnectivityMap = (payload) => {
  const items = Array.isArray(payload?.items) ? payload.items : []
  return Object.fromEntries(items.map((item) => [item.model_name, item]))
}

export const loadConnectivityMap = async (api) => {
  const data = await api.get('/api/model-connectivity')
  return buildConnectivityMap(data)
}
