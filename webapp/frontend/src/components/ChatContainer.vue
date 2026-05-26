<script setup>
import { inject, nextTick, ref, watch, reactive, computed } from 'vue'
import MarkdownIt from 'markdown-it'

const messages = inject('messages')
const isTyping = inject('isTyping')
const streamingContent = inject('streamingContent')
const isStreaming = inject('isStreaming')
const streamingModel = inject('streamingModel')
const currentModel = inject('currentModel')
const apiConfig = inject('apiConfig')
const liveUsage = inject('liveUsage')
const streamMeta = inject('streamMeta')
const voiceRuntime = inject('voiceRuntime')
const md = new MarkdownIt({ html: false, linkify: false })
const container = ref(null)
const processCollapsed = ref(false)

const detectProviderFromUrl = (url) => {
  if (!url) return ''
  const host = String(url).toLowerCase()
  if (host.includes('deepseek')) return 'DeepSeek'
  if (host.includes('siliconflow')) return 'SiliconFlow'
  if (host.includes('moonshot')) return 'Moonshot'
  if (host.includes('openai')) return 'OpenAI'
  return ''
}

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

const currentStepIndex = computed(() => {
  const plan = Array.isArray(streamMeta?.value?.plan) ? streamMeta.value.plan : []
  const current = (streamMeta?.value?.currentStep || '').trim()
  if (!current || plan.length === 0) return -1
  return plan.findIndex((s) => String(s || '').trim() === current)
})

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
    <div class="chat-status-bar">
      <span>模型: {{ (streamingModel || apiConfig?.effective_model_id || currentModel || '-').split('/').pop() }}</span>
      <span>供应商: {{ liveUsage?.provider || detectProviderFromUrl(apiConfig?.effective_api_url) || apiConfig?.effective_provider || '-' }}</span>
      <span>Token: {{ (liveUsage?.total || 0).toLocaleString() }} (↑{{ (liveUsage?.prompt || 0).toLocaleString() }} ↓{{ (liveUsage?.completion || 0).toLocaleString() }})</span>
      <span>Cache: R {{ (liveUsage?.cached_read || 0).toLocaleString() }} / W {{ (liveUsage?.cached_write || 0).toLocaleString() }}</span>
    </div>
    <div v-if="voiceRuntime?.enabled || (voiceRuntime?.phase && voiceRuntime.phase !== 'idle')" class="voice-runtime-bar">
      <span>语音会话: {{ voiceRuntime?.convId || '未绑定' }}</span>
      <span>阶段: {{ voiceRuntime?.phase || 'idle' }}</span>
      <span>来源: {{ voiceRuntime?.source || '-' }}</span>
      <span>队列: {{ voiceRuntime?.queueLength || 0 }}</span>
      <span>音频块: 收到 {{ voiceRuntime?.chunksReceived || 0 }} / 已播 {{ voiceRuntime?.chunksPlayed || 0 }}</span>
    </div>
    <div v-if="(streamMeta?.plan?.length || 0) > 0 || streamMeta?.currentStep || streamMeta?.audit || (streamMeta?.failover?.length || 0) > 0" class="process-panel">
      <button class="process-header" @click="processCollapsed = !processCollapsed">
        <span>过程面板</span>
        <span class="process-toggle">{{ processCollapsed ? '展开' : '收起' }}</span>
      </button>
      <div v-if="!processCollapsed">
        <div v-if="(streamMeta?.plan?.length || 0) > 0" class="process-line" style="align-items:flex-start;">
          <span class="process-label">计划</span>
          <ol class="process-plan">
            <li
              v-for="(step, idx) in streamMeta.plan"
              :key="`plan-${idx}`"
              :class="{ active: idx === currentStepIndex }"
            >
              {{ step }}
            </li>
          </ol>
        </div>
        <div v-if="streamMeta?.currentStep" class="process-line">
          <span class="process-label">步骤</span>
          <span>⏳ {{ streamMeta.currentStep }}</span>
        </div>
        <div v-if="streamMeta?.audit" class="process-line">
          <span class="process-label">自检</span>
          <span v-if="streamMeta.audit === 'ok'">通过</span>
          <span v-else>重试中{{ streamMeta?.auditReason ? `：${streamMeta.auditReason}` : '' }}</span>
        </div>
        <div v-if="(streamMeta?.failover?.length || 0) > 0" class="process-line" style="align-items:flex-start;">
          <span class="process-label">故障转移</span>
          <ul class="process-plan" style="padding-left:14px;">
            <li v-for="(f, idx) in streamMeta.failover" :key="'fo-'+idx">
              <template v-if="f.failover_type === 'auth_profile'">鉴权轮转 {{ f.from_profile }} → {{ f.to_profile }}</template>
              <template v-else-if="f.failover_type === 'model_fallback'">模型回退 {{ f.from_model }} → {{ f.to_model }}</template>
              <template v-else>回退耗尽（{{ f.error_class || 'unknown' }}）</template>
            </li>
          </ul>
        </div>
      </div>
    </div>

    <div v-if="messages.length === 0" style="display: flex; flex: 1; align-items: center; justify-content: center; color: var(--text-secondary); flex-direction: column; opacity: 0.5;">
      <h2 style="font-family: var(--font-display); margin-bottom: 0.5rem;">How can I help you?</h2>
      <p style="font-size: var(--font-sm);">Start a new conversation or select a skill.</p>
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
  font-size: var(--font-xs);
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
  font-size: var(--font-xs);
  color: var(--text-secondary);
  opacity: 0.6;
  margin-left: 4px;
  font-family: 'Fira Code', monospace;
}

.tool-chevron {
  transition: transform 0.2s ease;
  font-size: var(--font-md);
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
  font-size: var(--font-xs);
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
}

.voice-runtime-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
  margin: 0 0 0.8rem;
  padding: 0.55rem 0.75rem;
  border: 1px solid var(--border-color);
  border-radius: 0.75rem;
  background: rgba(20, 24, 34, 0.55);
  color: var(--text-secondary);
  font-size: var(--font-xs);
}

.voice-runtime-bar span {
  white-space: nowrap;
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
  font-size: var(--font-xs);
  color: var(--text-secondary);
  opacity: 0.45;
  margin-top: -0.35rem;
  padding-left: 4px;
  font-family: 'Fira Code', monospace;
  letter-spacing: 0.02em;
}

.chat-status-bar {
  position: sticky;
  top: 0;
  z-index: 4;
  display: flex;
  gap: 12px;
  align-items: center;
  flex-wrap: wrap;
  font-size: var(--font-xs);
  color: var(--text-secondary);
  font-family: 'Fira Code', monospace;
  background: color-mix(in srgb, var(--panel-bg) 88%, transparent);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 7px 10px;
  margin-bottom: 10px;
  backdrop-filter: blur(4px);
}

.process-panel {
  border: 1px dashed var(--border-color);
  border-radius: 8px;
  padding: 7px 10px;
  margin-bottom: 10px;
  font-size: var(--font-xs);
  color: var(--text-secondary);
  font-family: 'Fira Code', monospace;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.process-header {
  width: 100%;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0;
  cursor: pointer;
  font-family: 'Fira Code', monospace;
  font-size: var(--font-xs);
}

.process-toggle {
  opacity: 0.75;
}

.process-line {
  display: flex;
  gap: 8px;
  align-items: baseline;
}

.process-label {
  min-width: 30px;
  color: var(--text-primary);
  opacity: 0.8;
}

.process-plan {
  margin: 0;
  padding-left: 16px;
}

.process-plan li {
  margin: 0 0 3px 0;
  opacity: 0.8;
}

.process-plan li.active {
  color: var(--accent-color);
  font-weight: 700;
  opacity: 1;
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
