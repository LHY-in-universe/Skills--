<script setup>
import { ref, onMounted, onUnmounted, inject, watch } from 'vue'

const apiBase = inject('apiBase')
const messages = inject('messages')
const isTyping = inject('isTyping')

// State
const status = ref('idle') // idle, listening, processing, speaking
const transcribedText = ref('')
const ws = ref(null)
const audioContext = ref(null)
const audioQueue = ref([])
const isPlaying = ref(false)

// UI Helpers
const statusColors = {
  idle: '#666',
  listening: '#4CAF50',
  processing: '#FFC107',
  speaking: '#2196F3'
}

const connectWS = () => {
  const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const wsHost = apiBase.value ? apiBase.value.replace(/^https?:\/\//, '') : window.location.host
  const wsUrl = `${wsProtocol}//${wsHost}/api/voice/bridge`
  
  ws.value = new WebSocket(wsUrl)
  ws.value.binaryType = 'arraybuffer'

  ws.value.onopen = () => {
    console.log('Voice Bridge connected')
    startMic()
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
        break
        
      case 'text':
        // Part of the orchestrator stream
        status.value = 'speaking'
        break
        
      case 'audio_stream':
        // Decode base64 to arraybuffer and add to queue
        const binaryString = atob(msg.data)
        const bytes = new Uint8Array(binaryString.length)
        for (let i = 0; i < binaryString.length; i++) {
          bytes[i] = binaryString.charCodeAt(i)
        }
        audioQueue.value.push(bytes.buffer)
        if (!isPlaying.value) playNextInQueue()
        break
        
      case 'done':
        // End of AI turn
        setTimeout(() => {
          if (!isPlaying.value) {
            status.value = 'idle'
            transcribedText.value = ''
          }
        }, 2000)
        break

      case 'error':
        console.error('Voice Error:', msg.content)
        status.value = 'idle'
        break
    }
  }

  ws.value.onclose = () => {
    console.log('Voice Bridge disconnected. Reconnecting...')
    setTimeout(connectWS, 3000)
  }
}

const startMic = async () => {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    audioContext.value = new (window.AudioContext || window.webkitAudioContext)({ sampleRate: 16000 })
    
    const source = audioContext.value.createMediaStreamSource(stream)
    const processor = audioContext.value.createScriptProcessor(4096, 1, 1)

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

const playNextInQueue = async () => {
  if (audioQueue.value.length === 0) {
    isPlaying.value = false
    if (status.value === 'speaking') status.value = 'idle'
    return
  }

  isPlaying.value = true
  const buffer = audioQueue.value.shift()
  
  try {
    // Edge-TTS returns MP3. We need to decode it.
    const audioBuf = await audioContext.value.decodeAudioData(buffer)
    const source = audioContext.value.createBufferSource()
    source.buffer = audioBuf
    source.connect(audioContext.value.destination)
    source.onended = playNextInQueue
    source.start(0)
  } catch (e) {
    console.error('Failed to play audio chunk:', e)
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

onMounted(() => {
  connectWS()
})

onUnmounted(() => {
  if (ws.value) ws.value.close()
  if (audioContext.value) audioContext.value.close()
})
</script>

<template>
  <div class="voice-assistant-ship" :class="status">
    <div class="orb-container">
      <div class="orb" :style="{ backgroundColor: statusColors[status] }"></div>
      <div v-if="status === 'listening' || status === 'speaking'" class="pulses">
        <div class="pulse"></div>
        <div class="pulse"></div>
      </div>
    </div>
    
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
  position: fixed;
  bottom: 100px;
  right: 30px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  pointer-events: none;
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
  margin-top: 15px;
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
  font-size: 14px;
  line-height: 1.4;
}
</style>
