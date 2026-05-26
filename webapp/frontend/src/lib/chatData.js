import { mapModelsResponse, mergeHistoryMetadata } from './chatState'

export const fetchConfigData = async ({ api, apiConfig, currentModel }) => {
  const configRes = await api.get('/api/config')
  if (configRes) {
    apiConfig.value = configRes
    currentModel.value = configRes.current_model
  }
}

export const fetchHistoryData = async ({ api, messages }) => {
  const historyRes = await api.get('/api/history')
  messages.value = mergeHistoryMetadata(messages.value, historyRes)
}

export const fetchConversationsData = async ({ api, conversations, activeConversationId }) => {
  const res = await api.get('/api/conversations')
  conversations.value = res
  const active = res.find((item) => item.active)
  if (active) activeConversationId.value = active.id
}

export const fetchInitialAppData = async ({
  api,
  models,
  apiConfig,
  currentModel,
  skills,
  routingConfig,
}) => {
  const safe = (promise) => promise.catch((err) => {
    console.error('fetch error:', err)
    return null
  })
  const [modelsRes, configRes, skillsRes, routingRes] = await Promise.all([
    safe(api.get('/api/models')),
    safe(api.get('/api/config')),
    safe(api.get('/api/skills')),
    safe(api.get('/api/routing')),
  ])
  if (modelsRes) {
    models.value = mapModelsResponse(modelsRes)
  }
  if (configRes) {
    apiConfig.value = configRes
    currentModel.value = configRes.current_model
  }
  if (skillsRes) skills.value = skillsRes
  if (routingRes) routingConfig.value = routingRes
}
