export const parseSseLine = (line) => {
  if (!line.startsWith('data: ')) return null
  try {
    return JSON.parse(line.slice(6))
  } catch {
    return null
  }
}

export const applyUsageEvent = (liveUsage, event) => {
  liveUsage.value.prompt += event.prompt || 0
  liveUsage.value.completion += event.completion || 0
  liveUsage.value.total += event.total || ((event.prompt || 0) + (event.completion || 0))
  liveUsage.value.call_type = event.call_type || liveUsage.value.call_type || ''
  liveUsage.value.model = event.model || liveUsage.value.model || ''
  liveUsage.value.provider = event.provider || liveUsage.value.provider || ''
  liveUsage.value.cached_read += event.cached_read || 0
  liveUsage.value.cached_write += event.cached_write || 0
  return { prompt: event.prompt, completion: event.completion, total: event.total }
}

export const pushFailoverEvent = (streamMeta, event, exhausted = false) => {
  const arr = Array.isArray(streamMeta.value.failover) ? streamMeta.value.failover : []
  arr.push(exhausted ? { ...event, failover_type: 'exhausted' } : event)
  streamMeta.value.failover = arr.slice(-8)
}

export const applyStartEvent = ({ event, streamingModel, currentModel, apiConfig }) => {
  streamingModel.value = event._model || currentModel.value
  apiConfig.value = {
    ...(apiConfig.value || {}),
    current_model: event._model || currentModel.value,
    effective_model_id: event._model_id || apiConfig.value?.effective_model_id || '',
    effective_provider: event._provider || apiConfig.value?.effective_provider || '',
  }
}
