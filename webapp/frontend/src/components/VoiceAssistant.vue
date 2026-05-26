<script setup>
import { ref, onMounted, onUnmounted, inject, watch } from 'vue'

const apiBase = inject('apiBase')
const messages = inject('messages')
const isTyping = inject('isTyping')
const activeConversationId = inject('activeConversationId', ref(''))
const voiceRuntime = inject('voiceRuntime', ref({
  enabled: false,
  convId: '',
  phase: 'idle',
  source: '',
  queueLength: 0,
  chunksReceived: 0,
  chunksPlayed: 0,
}))

// State
const status = ref('idle') // idle, listening, processing, speaking
const transcribedText = ref('')
const ws = ref(null)
const audioContext = ref(null)
const audioQueue = ref([])
const isPlaying = ref(false)
const voiceEnabled = ref(false)
const debugBypassWakeword = ref(true)
const debugInjectText = ref('')
const voiceSession = ref({ convId: '', phase: 'idle', source: '' })
const micStream = ref(null)
const sourceNode = ref(null)
const processorNode = ref(null)
const voiceAssistantDraftIdx = ref(-1)
const isPanelOpen = ref(false)

// UI Helpers
const statusColors = {
  idle: '#666',
  listening: '#4CAF50',
  processing: '#FFC107',
  speaking: '#2196F3'
}

const connectWS = () => {
  if (!voiceEnabled.value) return
  const base = typeof apiBase === 'string' ? apiBase : (apiBase?.value || '')
  const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const wsHost = base
    ? base.replace(/^https?:\/\//, '')
    : ((window.location.port === '5173' || window.location.port === '5174')
      ? 'localhost:8000'
      : window.location.host)
  const wsUrl = `${wsProtocol}//${wsHost}/api/voice/bridge`
  
  ws.value = new WebSocket(wsUrl)
  ws.value.binaryType = 'arraybuffer'

  ws.value.onopen = () => {
    console.log('Voice Bridge connected:', wsUrl)
    ws.value.send(JSON.stringify({
      type: 'debug_config',
      bypass_wakeword: !!debugBypassWakeword.value,
      conv_id: activeConversationId?.value || '',
    }))
    startMic()
    voiceRuntime.value = {
      ...voiceRuntime.value,
      enabled: true,
      convId: activeConversationId?.value || '',
      phase: 'connecting',
      source: 'ws_open',
    }
  }
  ws.value.onerror = (e) => {
    console.error('Voice Bridge error:', e, wsUrl)
    status.value = 'error'
  }

  ws.value.onmessage = async (event) => {
    const msg = JSON.parse(event.data)
    
    switch (msg.type) {
      case 'wakeword':
        status.value = 'listening'
        transcribedText.value = '正在聆听...'
        // Visual feedback
        playNotificationTone(440, 0.1) // A4 tone
        break
        
      case 'asr_result':
        status.value = 'processing'
        transcribedText.value = msg.content
        if (msg.content?.trim()) {
          messages.value.push({ role: 'user', content: msg.content.trim() })
          isTyping.value = true
          voiceAssistantDraftIdx.value = -1
        }
        break
        
      case 'text':
        // Part of the orchestrator stream, keep writing into chat panel
        status.value = 'speaking'
        if (voiceAssistantDraftIdx.value < 0) {
          messages.value.push({ role: 'assistant', content: msg.content || '' })
          voiceAssistantDraftIdx.value = messages.value.length - 1
        } else {
          const cur = messages.value[voiceAssistantDraftIdx.value] || { role: 'assistant', content: '' }
          cur.content = (cur.content || '') + (msg.content || '')
          messages.value[voiceAssistantDraftIdx.value] = cur
        }
        break
        
      case 'audio_stream': {
        // Decode base64 to arraybuffer and add to queue
        const binaryString = atob(msg.data)
        const bytes = new Uint8Array(binaryString.length)
        for (let i = 0; i < binaryString.length; i++) {
          bytes[i] = binaryString.charCodeAt(i)
        }
        audioQueue.value.push(bytes.buffer)
        voiceRuntime.value = {
          ...voiceRuntime.value,
          queueLength: audioQueue.value.length,
          chunksReceived: (voiceRuntime.value.chunksReceived || 0) + 1,
        }
        if (!isPlaying.value) playNextInQueue()
        break
      }
        
      case 'done':
        // End of AI turn; keep continuous listening when voice is enabled
        setTimeout(() => {
          if (!isPlaying.value) {
            status.value = voiceEnabled.value ? 'listening' : 'idle'
            transcribedText.value = ''
          }
        }, 2000)
        isTyping.value = false
        voiceAssistantDraftIdx.value = -1
        break

      case 'error':
        console.error('Voice Error:', msg.content)
        status.value = voiceEnabled.value ? 'listening' : 'idle'
        isTyping.value = false
        break

      case 'voice_session_state':
        voiceSession.value = {
          convId: msg.conv_id || '',
          phase: msg.phase || '',
          source: msg.source || '',
        }
        voiceRuntime.value = {
          ...voiceRuntime.value,
          enabled: voiceEnabled.value,
          convId: msg.conv_id || '',
          phase: msg.phase || '',
          source: msg.source || '',
          queueLength: audioQueue.value.length,
        }
        break
      case 'debug_config_ack':
        console.log('voice debug config:', msg)
        break
    }
  }

  ws.value.onclose = () => {
    console.log('Voice Bridge disconnected. Reconnecting...')
    if (voiceEnabled.value) setTimeout(connectWS, 3000)
  }
}

const startMic = async () => {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    micStream.value = stream
    audioContext.value = new (window.AudioContext || window.webkitAudioContext)({ sampleRate: 16000 })
    
    const source = audioContext.value.createMediaStreamSource(stream)
    const processor = audioContext.value.createScriptProcessor(4096, 1, 1)
    sourceNode.value = source
    processorNode.value = processor

    source.connect(processor)
    processor.connect(audioContext.value.destination)

    processor.onaudioprocess = (e) => {
      if (ws.value && ws.value.readyState === WebSocket.OPEN) {
        const inputData = e.inputBuffer.getChannelData(0)
        // Convert float32 to pcm16
        const pcm16 = new Int16Array(inputData.length)
        for (let i = 0; i < inputData.length; i++) {
          const s = Math.max(-1, Math.min(1, inputData[i]))
          pcm16[i] = s < 0 ? s * 0x8000 : s * 0x7FFF
        }
        ws.value.send(pcm16.buffer)
      }
    }
  } catch (err) {
    console.error('Mic access denied:', err)
    status.value = 'error'
  }
}

const stopMic = () => {
  try {
    processorNode.value?.disconnect()
  } catch (err) {
    console.warn('Failed to disconnect processor node:', err)
  }
  try {
    sourceNode.value?.disconnect()
  } catch (err) {
    console.warn('Failed to disconnect source node:', err)
  }
  try {
    micStream.value?.getTracks().forEach((t) => t.stop())
  } catch (err) {
    console.warn('Failed to stop microphone tracks:', err)
  }
  processorNode.value = null
  sourceNode.value = null
  micStream.value = null
}

const disableVoice = async () => {
  voiceEnabled.value = false
  status.value = 'idle'
  transcribedText.value = ''
  voiceSession.value = { convId: '', phase: 'idle', source: '' }
  audioQueue.value = []
  voiceRuntime.value = {
    enabled: false,
    convId: '',
    phase: 'idle',
    source: 'disable_voice',
    queueLength: 0,
    chunksReceived: voiceRuntime.value.chunksReceived || 0,
    chunksPlayed: voiceRuntime.value.chunksPlayed || 0,
  }
  isPlaying.value = false
  stopMic()
  if (ws.value && ws.value.readyState === WebSocket.OPEN) {
    try {
      ws.value.close()
    } catch (err) {
      console.warn('Failed to close voice websocket:', err)
    }
  }
  ws.value = null
  if (audioContext.value) {
    try {
      await audioContext.value.close()
    } catch (err) {
      console.warn('Failed to close audio context:', err)
    }
  }
  audioContext.value = null
}

const enableVoice = () => {
  voiceEnabled.value = true
  status.value = 'listening'
  transcribedText.value = '语音输入已启动'
  voiceSession.value = {
    convId: activeConversationId?.value || '',
    phase: 'connecting',
    source: 'enable_voice',
  }
  voiceRuntime.value = {
    ...voiceRuntime.value,
    enabled: true,
    convId: activeConversationId?.value || '',
    phase: 'connecting',
    source: 'enable_voice',
  }
  connectWS()
}

const toggleVoiceInput = async () => {
  if (voiceEnabled.value) await disableVoice()
  else enableVoice()
}

const togglePanel = () => {
  isPanelOpen.value = !isPanelOpen.value
}

const applyDebugConfig = () => {
  if (!ws.value || ws.value.readyState !== WebSocket.OPEN) return
  ws.value.send(JSON.stringify({
    type: 'debug_config',
    bypass_wakeword: !!debugBypassWakeword.value,
    conv_id: activeConversationId?.value || '',
  }))
}

watch(activeConversationId, () => {
  voiceRuntime.value = {
    ...voiceRuntime.value,
    convId: activeConversationId?.value || '',
  }
  applyDebugConfig()
})

const endCurrentUtterance = () => {
  if (!voiceEnabled.value || !ws.value || ws.value.readyState !== WebSocket.OPEN) return
  status.value = 'processing'
  transcribedText.value = '正在结束本次语音并识别...'
  ws.value.send(JSON.stringify({ type: 'end_utterance' }))
}

const sendDebugInjectText = () => {
  const text = String(debugInjectText.value || '').trim()
  if (!voiceEnabled.value || !text || !ws.value || ws.value.readyState !== WebSocket.OPEN) return
  status.value = 'processing'
  transcribedText.value = text
  ws.value.send(JSON.stringify({
    type: 'debug_inject_text',
    content: text,
    conv_id: activeConversationId?.value || '',
  }))
}

const abortVoiceChat = () => {
  if (!ws.value || ws.value.readyState !== WebSocket.OPEN) return
  ws.value.send(JSON.stringify({ type: 'abort' }))
  status.value = voiceEnabled.value ? 'listening' : 'idle'
  isTyping.value = false
}

const playNextInQueue = async () => {
  if (audioQueue.value.length === 0) {
    isPlaying.value = false
    voiceRuntime.value = {
      ...voiceRuntime.value,
      queueLength: 0,
    }
    if (status.value === 'speaking') status.value = voiceEnabled.value ? 'listening' : 'idle'
    return
  }

  isPlaying.value = true
  const buffer = audioQueue.value.shift()
  voiceRuntime.value = {
    ...voiceRuntime.value,
    queueLength: audioQueue.value.length,
  }
  
  try {
    // Edge-TTS returns MP3. We need to decode it.
    const audioBuf = await audioContext.value.decodeAudioData(buffer)
    const source = audioContext.value.createBufferSource()
    source.buffer = audioBuf
    source.connect(audioContext.value.destination)
    source.onended = () => {
      voiceRuntime.value = {
        ...voiceRuntime.value,
        chunksPlayed: (voiceRuntime.value.chunksPlayed || 0) + 1,
        queueLength: audioQueue.value.length,
      }
      playNextInQueue()
    }
    source.start(0)
  } catch (e) {
    console.error('Failed to play audio chunk:', e)
    voiceRuntime.value = {
      ...voiceRuntime.value,
      queueLength: audioQueue.value.length,
    }
    playNextInQueue()
  }
}

const playNotificationTone = (freq, duration) => {
  if (!audioContext.value) return
  const osc = audioContext.value.createOscillator()
  const gain = audioContext.value.createGain()
  osc.connect(gain)
  gain.connect(audioContext.value.destination)
  osc.frequency.value = freq
  gain.gain.setValueAtTime(0, audioContext.value.currentTime)
  gain.gain.linearRampToValueAtTime(0.1, audioContext.value.currentTime + 0.05)
  gain.gain.linearRampToValueAtTime(0, audioContext.value.currentTime + duration)
  osc.start()
  osc.stop(audioContext.value.currentTime + duration)
}

onMounted(() => {})

onUnmounted(() => {
  disableVoice()
})
</script>

<template>
  <div class="voice-assistant-ship" :class="status">
    <div class="voice-anchor">
      <button class="voice-menu-btn" @click="togglePanel">
        <span class="voice-menu-text">语音</span>
        <span class="voice-menu-state">{{ isPanelOpen ? '收起' : '展开' }}</span>
      </button>
      <div class="orb-container">
        <div class="orb" :style="{ backgroundColor: statusColors[status] }"></div>
        <div v-if="status === 'listening' || status === 'speaking'" class="pulses">
          <div class="pulse"></div>
          <div class="pulse"></div>
        </div>
      </div>
    </div>

    <Transition name="voice-panel-fade">
      <div v-if="isPanelOpen" class="voice-panel">
        <button class="voice-toggle-btn" @click="toggleVoiceInput">
          {{ voiceEnabled ? '停止语音输入' : '启动语音输入' }}
        </button>
        <label class="voice-debug-toggle">
          <input type="checkbox" v-model="debugBypassWakeword" @change="applyDebugConfig" />
          调试模式（跳过唤醒词）
        </label>
        <button class="voice-end-btn" :disabled="!voiceEnabled" @click="endCurrentUtterance">
          结束本次语音
        </button>
        <button class="voice-end-btn danger" :disabled="!voiceEnabled" @click="abortVoiceChat">
          中断语音对话
        </button>
        <div v-if="debugBypassWakeword" class="voice-debug-box">
          <input
            v-model="debugInjectText"
            class="voice-debug-input"
            type="text"
            placeholder="调试注入文本，不用说话也能测试整条链"
            @keydown.enter.prevent="sendDebugInjectText"
          />
          <button class="voice-debug-send" :disabled="!voiceEnabled || !debugInjectText.trim()" @click="sendDebugInjectText">
            注入文本
          </button>
        </div>
        <div v-if="voiceEnabled" class="voice-session-meta">
          <span>会话: {{ voiceSession.convId || activeConversationId || '未绑定' }}</span>
          <span>阶段: {{ voiceSession.phase || status }}</span>
          <span>来源: {{ voiceSession.source || 'ui' }}</span>
        </div>
      </div>
    </Transition>
    
    <div v-if="transcribedText" class="voice-overlay">
      <div class="glossy-card">
        <span class="status-dot" :style="{ backgroundColor: statusColors[status] }"></span>
        <p>{{ transcribedText }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.voice-assistant-ship {
  position: absolute;
  right: 24px;
  bottom: 24px;
  z-index: 30;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  pointer-events: none;
  gap: 10px;
}

.voice-anchor {
  display: flex;
  align-items: center;
  gap: 10px;
  pointer-events: auto;
}

.voice-menu-btn {
  border: 1px solid rgba(255,255,255,0.18);
  background: rgba(0,0,0,0.6);
  color: #fff;
  border-radius: 10px;
  padding: 8px 12px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 10px;
}

.voice-menu-text {
  font-size: var(--font-xs);
  font-weight: 700;
}

.voice-menu-state {
  font-size: var(--font-xs);
  color: rgba(255,255,255,0.68);
}

.voice-panel {
  pointer-events: auto;
  width: 320px;
  display: flex;
  flex-direction: column;
  align-items: stretch;
}

.voice-panel-fade-enter-active,
.voice-panel-fade-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.voice-panel-fade-enter-from,
.voice-panel-fade-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

.voice-toggle-btn {
  pointer-events: auto;
  margin-bottom: 10px;
  border: 1px solid rgba(255,255,255,0.2);
  background: rgba(0,0,0,0.65);
  color: #fff;
  border-radius: 8px;
  padding: 8px 12px;
  font-size: var(--font-xs);
  cursor: pointer;
}

.voice-debug-toggle {
  pointer-events: auto;
  margin-bottom: 8px;
  color: #fff;
  font-size: var(--font-xs);
  background: rgba(0,0,0,0.45);
  border: 1px solid rgba(255,255,255,0.15);
  border-radius: 8px;
  padding: 6px 10px;
}
.voice-debug-toggle input {
  margin-right: 6px;
}

.voice-end-btn {
  pointer-events: auto;
  margin-bottom: 8px;
  border: 1px solid rgba(255,255,255,0.2);
  background: rgba(59,130,246,0.65);
  color: #fff;
  border-radius: 8px;
  padding: 7px 12px;
  font-size: var(--font-xs);
  cursor: pointer;
}
.voice-end-btn.danger {
  background: rgba(220,38,38,0.72);
}
.voice-end-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.voice-debug-box {
  pointer-events: auto;
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
  width: 320px;
}

.voice-debug-input {
  flex: 1;
  border: 1px solid rgba(255,255,255,0.18);
  background: rgba(0,0,0,0.55);
  color: #fff;
  border-radius: 8px;
  padding: 8px 10px;
  font-size: var(--font-xs);
}

.voice-debug-send {
  border: 1px solid rgba(255,255,255,0.2);
  background: rgba(16,185,129,0.72);
  color: #fff;
  border-radius: 8px;
  padding: 8px 12px;
  font-size: var(--font-xs);
  cursor: pointer;
}

.voice-debug-send:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.voice-session-meta {
  pointer-events: auto;
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 10px;
  max-width: 320px;
}

.voice-session-meta span {
  color: #fff;
  font-size: var(--font-xs);
  line-height: 1.2;
  background: rgba(0,0,0,0.45);
  border: 1px solid rgba(255,255,255,0.12);
  border-radius: 999px;
  padding: 5px 8px;
}

.orb-container {
  position: relative;
  width: 60px;
  height: 60px;
  display: flex;
  justify-content: center;
  align-items: center;
}

.orb {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  box-shadow: 0 0 20px rgba(0,0,0,0.3);
  transition: all 0.5s cubic-bezier(0.175, 0.885, 0.32, 1.275);
  border: 3px solid rgba(255,255,255,0.2);
}

.voice-assistant-ship.listening .orb {
  width: 50px;
  height: 50px;
  box-shadow: 0 0 30px v-bind('statusColors.listening');
}

.voice-assistant-ship.speaking .orb {
  box-shadow: 0 0 30px v-bind('statusColors.speaking');
}

.pulses {
  position: absolute;
  width: 100%;
  height: 100%;
}

.pulse {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 100%;
  height: 100%;
  border-radius: 50%;
  border: 2px solid v-bind('statusColors[status]');
  animation: ripple 2s infinite ease-out;
  opacity: 0;
}

.pulse:nth-child(2) {
  animation-delay: 1s;
}

@keyframes ripple {
  0% { width: 40px; height: 40px; opacity: 0.5; }
  100% { width: 100px; height: 100px; opacity: 0; }
}

.voice-overlay {
  margin-top: 2px;
  max-width: 300px;
  pointer-events: auto;
}

.glossy-card {
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.2);
  padding: 12px 20px;
  border-radius: 16px;
  color: white;
  box-shadow: 0 8px 32px rgba(0,0,0,0.3);
  display: flex;
  align-items: center;
  gap: 12px;
  animation: slideIn 0.3s ease-out;
}

@keyframes slideIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.glossy-card p {
  margin: 0;
  font-size: var(--font-sm);
  line-height: 1.4;
}
</style>
