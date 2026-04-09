<script setup>
import { inject, ref, computed } from 'vue'
import { Send, Square } from 'lucide-vue-next'

const emit = defineEmits(['send', 'abort'])
const isTyping = inject('isTyping')
const models = inject('models')
const clearHistoryFn = inject('clearHistoryFn')
const createConversationFn = inject('createConversationFn')
const exportConversation = inject('exportConversation')
const switchModel = inject('switchModel')

const text = ref('')
const textarea = ref(null)

// ── Input history ──────────────────────────────────────────
const history = ref([])
const historyIndex = ref(-1)
const historySaved = ref('')

// ── Slash menu ─────────────────────────────────────────────
const slashIndex = ref(0)

const slashItems = computed(() => {
  if (!text.value.startsWith('/')) return []
  const query = text.value.slice(1).toLowerCase()
  const base = [
    { label: '/clear',  desc: '清空当前对话',    run: () => clearHistoryFn?.() },
    { label: '/new',    desc: '新建对话',          run: () => createConversationFn?.() },
    { label: '/export', desc: '导出为 Markdown',  run: () => exportConversation?.() },
  ]
  const modelItems = Object.entries(models?.value || {}).map(([name, id]) => ({
    label: `/model ${name}`,
    desc: id,
    run: () => switchModel?.(id)
  }))
  return [...base, ...modelItems].filter(item =>
    item.label.slice(1).toLowerCase().includes(query)
  )
})

const showSlash = computed(() => slashItems.value.length > 0)

// ── Send ───────────────────────────────────────────────────
const handleSend = () => {
  if (!text.value.trim() || isTyping.value) return

  // Execute slash command
  if (showSlash.value) {
    slashItems.value[slashIndex.value]?.run()
    text.value = ''
    slashIndex.value = 0
    if (textarea.value) { textarea.value.style.height = 'auto'; textarea.value.focus() }
    return
  }

  // Record history (skip duplicates at top)
  const msg = text.value.trim()
  if (msg && history.value[history.value.length - 1] !== msg) {
    history.value.push(msg)
    if (history.value.length > 50) history.value.shift()
  }
  historyIndex.value = -1
  historySaved.value = ''

  emit('send', text.value)
  text.value = ''
  if (textarea.value) { textarea.value.style.height = 'auto'; textarea.value.focus() }
}

// ── Keydown ────────────────────────────────────────────────
const handleKeydown = (e) => {
  // Shift+Enter → send
  if (e.key === 'Enter' && e.shiftKey) { e.preventDefault(); handleSend(); return }

  // Slash menu navigation
  if (showSlash.value) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      slashIndex.value = (slashIndex.value + 1) % slashItems.value.length
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      slashIndex.value = (slashIndex.value - 1 + slashItems.value.length) % slashItems.value.length
    } else if (e.key === 'Escape') {
      text.value = ''
    } else if (e.key === 'Tab' || e.key === 'Enter') {
      e.preventDefault(); handleSend()
    }
    return
  }

  // Input history navigation (only when content has no newlines)
  if (!text.value.includes('\n')) {
    if (e.key === 'ArrowUp' && textarea.value?.selectionStart === 0) {
      e.preventDefault()
      if (historyIndex.value === -1) {
        historySaved.value = text.value
        historyIndex.value = history.value.length - 1
      } else if (historyIndex.value > 0) {
        historyIndex.value--
      }
      if (historyIndex.value >= 0) {
        text.value = history.value[historyIndex.value]
        setTimeout(() => {
          const len = text.value.length
          textarea.value?.setSelectionRange(len, len)
        }, 0)
      }
      return
    }
    if (e.key === 'ArrowDown' && historyIndex.value >= 0) {
      e.preventDefault()
      historyIndex.value++
      if (historyIndex.value >= history.value.length) {
        historyIndex.value = -1
        text.value = historySaved.value
        historySaved.value = ''
      } else {
        text.value = history.value[historyIndex.value]
      }
      setTimeout(() => {
        const len = text.value.length
        textarea.value?.setSelectionRange(len, len)
      }, 0)
    }
  }
}

// Auto-resize
const onInput = (e) => {
  e.target.style.height = 'auto'
  e.target.style.height = e.target.scrollHeight + 'px'
  slashIndex.value = 0
}
</script>

<template>
  <div class="input-area">
    <div class="input-wrapper" style="position: relative;">

      <!-- Slash command menu -->
      <Transition name="slash-fade">
        <div v-if="showSlash" class="slash-menu">
          <div
            v-for="(item, i) in slashItems"
            :key="item.label"
            class="slash-item"
            :class="{ active: i === slashIndex }"
            @mousedown.prevent="item.run(); text = ''; slashIndex = 0; textarea?.focus()"
          >
            <span class="slash-cmd">{{ item.label }}</span>
            <span class="slash-desc">{{ item.desc }}</span>
          </div>
        </div>
      </Transition>

      <textarea
        v-model="text"
        ref="textarea"
        placeholder="输入消息（Shift+Enter 发送）或输入 / 使用命令..."
        rows="1"
        @input="onInput"
        @keydown="handleKeydown"
      ></textarea>

      <button v-if="isTyping" class="abort-btn" @click="emit('abort')" title="停止生成">
        <Square size="16" />
      </button>
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
      Shift+Enter 发送 · / 命令 · ↑↓ 历史
    </p>
  </div>
</template>

<style scoped>
.slash-menu {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  right: 48px;
  background: var(--msg-assistant-bg, #1e1e2e);
  border: 1px solid var(--border-color, #333);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 -4px 16px rgba(0,0,0,0.3);
  z-index: 100;
}

.slash-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
  transition: background 0.15s;
}

.slash-item:hover,
.slash-item.active {
  background: rgba(99, 102, 241, 0.15);
}

.slash-cmd {
  font-family: 'Fira Code', monospace;
  font-size: 12px;
  font-weight: 600;
  color: var(--accent-color, #6366f1);
  white-space: nowrap;
  min-width: 130px;
}

.slash-desc {
  font-size: 11px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slash-fade-enter-active,
.slash-fade-leave-active {
  transition: opacity 0.15s, transform 0.15s;
}
.slash-fade-enter-from,
.slash-fade-leave-to {
  opacity: 0;
  transform: translateY(4px);
}
</style>
