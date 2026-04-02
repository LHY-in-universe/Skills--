<script setup>
import { inject } from 'vue'
import { Send, Square } from 'lucide-vue-next'
import { ref } from 'vue'

const emit = defineEmits(['send', 'abort'])
const isTyping = inject('isTyping')
const text = ref('')
const textarea = ref(null)

const handleSend = () => {
  if (!text.value.trim() || isTyping.value) return
  emit('send', text.value)
  text.value = ''
  if (textarea.value) {
    textarea.value.style.height = 'auto'
    textarea.value.focus()
  }
}

// Shift+Enter → send; Enter → newline (default browser behavior)
const handleKeydown = (e) => {
  if (e.key === 'Enter' && e.shiftKey) {
    e.preventDefault()
    handleSend()
  }
}

// Auto-resize textarea
const onInput = (e) => {
  e.target.style.height = 'auto'
  e.target.style.height = e.target.scrollHeight + 'px'
}
</script>

<template>
  <div class="input-area">
    <div class="input-wrapper">
      <textarea
        v-model="text"
        ref="textarea"
        placeholder="输入消息（Shift+Enter 发送，Enter 换行）..."
        rows="1"
        @input="onInput"
        @keydown="handleKeydown"
      ></textarea>

      <!-- Stop button when AI is responding -->
      <button
        v-if="isTyping"
        class="abort-btn"
        @click="emit('abort')"
        title="停止生成"
      >
        <Square size="16" />
      </button>

      <!-- Send button when idle -->
      <button
        v-else
        class="send-btn"
        @click="handleSend"
        :style="{ opacity: text.trim() ? 1 : 0.4, cursor: text.trim() ? 'pointer' : 'not-allowed' }"
      >
        <Send size="18" />
      </button>
    </div>
    <p style="text-align: center; font-size: 11px; color: var(--text-secondary); margin-top: 10px; opacity: 0.5;">
      Shift+Enter 发送 · AI 可能犯错，请自行核实重要信息
    </p>
  </div>
</template>
