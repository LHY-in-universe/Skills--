<script setup>
import { inject, nextTick, ref, watch, reactive } from 'vue'
import MarkdownIt from 'markdown-it'

const messages = inject('messages')
const isTyping = inject('isTyping')
const streamingContent = inject('streamingContent')
const isStreaming = inject('isStreaming')
const streamingModel = inject('streamingModel')
const md = new MarkdownIt({ html: false, linkify: false })
const container = ref(null)

// Track expanded state for tool messages by index
const expandedTools = reactive({})

const toggleTool = (index) => {
  expandedTools[index] = !expandedTools[index]
}

const getToolLabel = (msg, index, toolCall = null) => {
  if (msg.role === 'tool') return `🔧 工具返回结果`
  if (msg.role === 'assistant' && toolCall) {
    const name = toolCall.function?.name || 'tool'
    return `⚙️ 调用 ${name}...`
  }
  return `工具消息 #${index}`
}

const renderMarkdown = (content) => {
  return md.render(content || '')
}

const scrollToBottom = async () => {
  await nextTick()
  if (container.value) {
    container.value.scrollTop = container.value.scrollHeight
  }
}

watch(() => messages.value.length, scrollToBottom)
watch(() => isTyping.value, scrollToBottom)
watch(() => streamingContent.value, scrollToBottom)
</script>

<template>
  <div class="messages-container" ref="container">
    <div v-if="messages.length === 0" style="display: flex; flex: 1; align-items: center; justify-content: center; color: var(--text-secondary); flex-direction: column; opacity: 0.5;">
      <h2 style="font-family: var(--font-display); margin-bottom: 0.5rem;">How can I help you?</h2>
      <p style="font-size: 0.9rem;">Start a new conversation or select a skill.</p>
    </div>

    <template v-for="(msg, index) in messages" :key="index">
      
      <!-- 1. Text Content: Only render if there is non-whitespace text, skipping empty messages -->
      <div
        v-if="msg.content && msg.content.trim() !== '' && msg.role !== 'tool'"
        :class="['message', msg.role]"
        v-html="renderMarkdown(msg.content)"
      ></div>

      <!-- Model label + token count under assistant messages -->
      <div
        v-if="msg.role === 'assistant' && !msg.tool_calls && msg.content?.trim() && (msg._model || msg._tokens)"
        class="model-label"
      >
        <span v-if="msg._model">{{ msg._model.split('/').pop() }}</span>
        <span v-if="msg._tokens" class="msg-tokens">· {{ msg._tokens.total.toLocaleString() }} tokens (↑{{ msg._tokens.prompt }} ↓{{ msg._tokens.completion }})</span>
      </div>

      <!-- 2. Assistant Tool Calls -->
      <template v-if="msg.role === 'assistant' && msg.tool_calls">
        <div v-for="(tc, tcIndex) in msg.tool_calls" :key="'tc-'+index+'-'+tcIndex" class="tool-message-wrapper">
          <button class="tool-collapse-btn" @click="toggleTool(index + '-' + tcIndex)">
            <span class="tool-label">{{ getToolLabel(msg, index, tc) }}</span>
            <span class="tool-chevron" :class="{ expanded: expandedTools[index + '-' + tcIndex] }">▾</span>
          </button>
          <div v-if="expandedTools[index + '-' + tcIndex]" class="tool-content">
            <pre>{{ JSON.stringify(tc.function || tc, null, 2) }}</pre>
          </div>
        </div>
      </template>

      <!-- 3. Tool Execution Results -->
      <div v-if="msg.role === 'tool'" class="tool-message-wrapper">
        <button class="tool-collapse-btn" @click="toggleTool(index)">
          <span class="tool-label">{{ getToolLabel(msg, index) }}</span>
          <span class="tool-chevron" :class="{ expanded: expandedTools[index] }">▾</span>
        </button>
        <div v-if="expandedTools[index]" class="tool-content">
          <pre>{{ msg.content }}</pre>
        </div>
      </div>

    </template>

    <!-- Streaming content (live text as it arrives) -->
    <template v-if="isStreaming && streamingContent">
      <div class="message assistant streaming" v-html="renderMarkdown(streamingContent)"></div>
      <div v-if="streamingModel" class="model-label">{{ streamingModel.split('/').pop() }}</div>
    </template>

    <!-- Typing indicator: shown when waiting but no text yet -->
    <template v-else-if="isTyping">
      <div class="message assistant" style="display: flex; align-items: center; gap: 8px; opacity: 0.7;">
        <div class="dot-typing">
          <span></span><span></span><span></span>
        </div>
      </div>
      <div v-if="streamingModel" class="model-label">{{ streamingModel.split('/').pop() }}</div>
    </template>
  </div>
</template>

<style scoped>
/* Tool message collapsible */
.tool-message-wrapper {
  align-self: flex-start;
  max-width: 85%;
  margin: -0.5rem 0;
}

.tool-collapse-btn {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  background: transparent;
  border: 1px dashed var(--border-color);
  border-radius: 0.5rem;
  padding: 0.4rem 0.75rem;
  color: var(--text-secondary);
  font-size: 0.78rem;
  cursor: pointer;
  width: 100%;
  text-align: left;
  transition: border-color 0.2s, color 0.2s, background 0.2s;
}

.tool-collapse-btn:hover {
  border-color: var(--accent-color);
  color: var(--accent-color);
  background: rgba(99, 102, 241, 0.05);
}

.tool-label { flex: 1; }

.msg-tokens {
  font-size: 10px;
  color: var(--text-secondary);
  opacity: 0.6;
  margin-left: 4px;
  font-family: 'Fira Code', monospace;
}

.tool-chevron {
  transition: transform 0.2s ease;
  font-size: 1rem;
}
.tool-chevron.expanded {
  transform: rotate(180deg);
}

.tool-content {
  margin-top: 4px;
  border: 1px solid var(--border-color);
  border-radius: 0.5rem;
  overflow: auto;
  max-height: 300px;
  animation: fadeIn 0.2s ease-out;
}

.tool-content pre {
  padding: 0.75rem 1rem;
  font-family: 'Fira Code', monospace;
  font-size: 0.75rem;
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
}

/* Streaming cursor */
.message.streaming::after {
  content: '▋';
  display: inline-block;
  animation: blink 0.7s step-end infinite;
  color: var(--accent-color);
  margin-left: 1px;
}
@keyframes blink {
  0%, 100% { opacity: 1; }
  50%       { opacity: 0; }
}

/* Model label */
.model-label {
  align-self: flex-start;
  font-size: 10px;
  color: var(--text-secondary);
  opacity: 0.45;
  margin-top: -0.35rem;
  padding-left: 4px;
  font-family: 'Fira Code', monospace;
  letter-spacing: 0.02em;
}

/* Typing dots */
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
