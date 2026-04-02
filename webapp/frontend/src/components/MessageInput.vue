<script setup>
import { ref, defineEmits, inject } from 'vue'
import { Send } from 'lucide-vue-next'

const emit = defineEmits(['send'])
const isTyping = inject('isTyping')
const text = ref('')
const textarea = ref(null)

const handleSend = () => {
  if (!text.value.trim() || isTyping.value) return
  emit('send', text.value)
  text.value = ''
  if (textarea.value) textarea.value.focus()
}

const handleKeydown = (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    handleSend()
  }
}

// Auto-resize textarea
const onInput = (e) => {
  e.target.style.height = 'auto'
  e.target.style.height = (e.target.scrollHeight) + 'px'
}

</script>

<template>
  <div class="input-area">
    <div class="input-wrapper">
      <textarea 
        v-model="text"
        ref="textarea"
        placeholder="Type a message or ask for a skill..." 
        rows="1"
        @input="onInput"
        @keydown="handleKeydown"
        :disabled="isTyping"
      ></textarea>
      <button 
        class="send-btn" 
        @click="handleSend"
        :style="{ opacity: (text.trim() && !isTyping) ? 1 : 0.5, cursor: (text.trim() && !isTyping) ? 'pointer' : 'not-allowed' }"
      >
        <Send size="18" />
      </button>
    </div>
    <p style="text-align: center; font-size: 11px; color: var(--text-secondary); margin-top: 10px; opacity: 0.5;">
      AI can make mistakes. Please verify important information.
    </p>
  </div>
</template>
