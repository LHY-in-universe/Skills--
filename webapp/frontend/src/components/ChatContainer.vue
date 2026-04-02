<script setup>
import { inject, nextTick, ref, watch } from 'vue'
import MarkdownIt from 'markdown-it'

const messages = inject('messages')
const isTyping = inject('isTyping')
const md = new MarkdownIt()
const container = ref(null)

const renderMarkdown = (content) => {
  return md.render(content)
}

const scrollToBottom = async () => {
    await nextTick()
    if (container.value) {
        container.value.scrollTop = container.value.scrollHeight
    }
}

// Watch for message changes to scroll
watch(() => messages.value.length, scrollToBottom)
watch(() => isTyping.value, scrollToBottom)

</script>

<template>
  <div class="messages-container" ref="container">
    <div v-if="messages.length === 0" style="display: flex; flex: 1; align-items: center; justify-content: center; color: var(--text-secondary); flex-direction: column; opacity: 0.5;">
      <h2 style="font-family: var(--font-display); margin-bottom: 0.5rem;">How can I help you?</h2>
      <p style="font-size: 0.9rem;">Start a new conversation or select a skill.</p>
    </div>
    
    <div 
      v-for="(msg, index) in messages" 
      :key="index" 
      :class="['message', msg.role]"
      v-html="renderMarkdown(msg.content || '')"
    ></div>

    <div v-if="isTyping" class="message assistant" style="display: flex; align-items: center; gap: 8px; opacity: 0.7;">
      <div class="dot-typing">
        <span></span><span></span><span></span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dot-typing {
  display: flex;
  gap: 4px;
}
.dot-typing span {
  width: 6px;
  height: 6px;
  background-color: var(--text-primary);
  border-radius: 50%;
  animation: bounce 1.4s infinite ease-in-out both;
}
.dot-typing span:nth-child(1) { animation-delay: -0.32s; }
.dot-typing span:nth-child(2) { animation-delay: -0.16s; }

@keyframes bounce {
  0%, 80%, 100% { transform: scale(0); }
  40% { transform: scale(1.0); }
}
</style>
