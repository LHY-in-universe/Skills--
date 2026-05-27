import { createLiveUsage, createStreamMeta } from './chatState'
import {
  applyStartEvent,
  applyUsageEvent,
  parseSseLine,
  pushFailoverEvent,
} from './chatStream'

export const createChatRuntime = ({
  api,
  nextTick,
  currentModel,
  activeConversationId,
  messages,
  isTyping,
  streamingContent,
  isStreaming,
  streamingModel,
  apiConfig,
  lastRouteInfo,
  liveUsage,
  streamMeta,
  permissionDialog,
  fetchHistory,
  resetStreamingState,
}) => {
  let abortController = null

  const abortActiveRequest = () => {
    if (!abortController) return
    try {
      abortController.abort()
    } catch (err) {
      console.warn('Abort controller cleanup failed:', err)
    }
  }

  const processStream = async (response) => {
    const reader = response.body.getReader()
    const decoder = new TextDecoder()
    let buffer = ''
    let respondingModel = currentModel.value
    let pendingUsage = null

    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split('\n')
      buffer = lines.pop() ?? ''

      for (const line of lines) {
        const event = parseSseLine(line)
        if (!event) continue

        switch (event.type) {
          case 'plan':
            streamMeta.value.plan = Array.isArray(event.steps) ? event.steps : []
            break
          case 'step_start':
            streamMeta.value.currentStep = event.step || ''
            break
          case 'step_done':
            streamMeta.value.currentStep = ''
            break
          case 'audit':
            streamMeta.value.audit = event.ok ? 'ok' : 'retry'
            if (event.ok === false && event.reason) streamMeta.value.auditReason = event.reason
            break
          case 'failover_step':
            pushFailoverEvent(streamMeta, event)
            break
          case 'failover_exhausted':
            pushFailoverEvent(streamMeta, event, true)
            break
          case 'start':
            applyStartEvent({ event, streamingModel, currentModel, apiConfig })
            break
          case 'text':
            streamingContent.value += event.content
            await nextTick()
            break
          case 'usage':
            pendingUsage = applyUsageEvent(liveUsage, event)
            break
          case 'permission_required':
            resetStreamingState({ keepTyping: true })
            permissionDialog.value = {
              visible: true,
              toolName: event.tool_name,
              description: event.description,
            }
            await fetchHistory()
            return { status: 'permission' }
          case 'aborted':
            resetStreamingState()
            await fetchHistory()
            return { status: 'aborted' }
          case 'error':
            resetStreamingState()
            return {
              status: 'error',
              message: event.content || 'Unknown stream error',
              errorClass: event.error_class || '',
            }
          case 'done': {
            respondingModel = event._model || streamingModel.value || currentModel.value
            if (event._tier) lastRouteInfo.value = { tier: event._tier, model: respondingModel }
            const prevLen = messages.value.length
            await fetchHistory()
            for (let i = prevLen; i < messages.value.length; i += 1) {
              const msg = messages.value[i]
              if (msg.role === 'assistant' && !msg.tool_calls && msg.content?.trim()) {
                messages.value[i] = {
                  ...msg,
                  _model: respondingModel,
                  ...(pendingUsage ? { _tokens: pendingUsage } : {}),
                }
              }
            }
            pendingUsage = null
            resetStreamingState()
            break
          }
        }
      }
    }
    return { status: 'done' }
  }

  const sendMessage = async (text) => {
    if (!text.trim()) return
    abortActiveRequest()
    messages.value.push({ role: 'user', content: text })
    isTyping.value = true
    isStreaming.value = true
    streamingContent.value = ''
    streamMeta.value = createStreamMeta()
    liveUsage.value = createLiveUsage()
    let retried = false
    while (true) {
      abortController = new AbortController()
      try {
        const response = await api.stream('/api/chat', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ user_input: text, conv_id: activeConversationId.value || undefined }),
          signal: abortController.signal,
        })
        const result = await processStream(response)
        if (result?.status === 'error') {
          const msg = String(result.message || '')
          const errClass = result.errorClass ? ` (${result.errorClass})` : ''
          const isConnErr = msg.toLowerCase().includes('connection error')
          if (isConnErr && !retried) {
            retried = true
            isTyping.value = true
            isStreaming.value = true
            streamingContent.value = ''
            continue
          }
          messages.value.push({ role: 'assistant', content: `**Error${errClass}:** ${msg}` })
        }
        break
      } catch (err) {
        resetStreamingState({ keepTyping: err.name === 'AbortError' })
        if (err.name !== 'AbortError') {
          messages.value.push({
            role: 'assistant',
            content: `**Error:** ${err.message}. Please check your API configuration.`,
          })
        }
        break
      }
    }
  }

  const handlePermissionResponse = async (granted, alwaysAllow = false) => {
    permissionDialog.value.visible = false
    isTyping.value = true
    isStreaming.value = true
    streamingContent.value = ''
    abortController = new AbortController()
    try {
      const response = await api.stream('/api/chat/resume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          granted,
          always_allow: alwaysAllow,
          conv_id: activeConversationId.value || undefined,
        }),
        signal: abortController.signal,
      })
      const result = await processStream(response)
      if (result?.status === 'error') {
        messages.value.push({ role: 'assistant', content: `**Error:** ${result.message || 'Resume failed'}` })
      }
    } catch (err) {
      resetStreamingState({ keepTyping: err.name === 'AbortError' })
      if (err.name !== 'AbortError') {
        messages.value.push({ role: 'assistant', content: `**Error:** ${err.message}` })
      }
    }
  }

  const abortChat = async () => {
    abortActiveRequest()
    isTyping.value = false
    permissionDialog.value.visible = false
    await api.post('/api/chat/abort', { conv_id: activeConversationId.value || undefined }).catch(() => {})
  }

  return {
    sendMessage,
    handlePermissionResponse,
    abortChat,
  }
}
