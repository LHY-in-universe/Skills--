<script setup>
import { inject, onMounted, ref } from 'vue'

const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))

const terminalCwd = ref('')

const loadTerminalCwd = async () => {
  try {
    const res = await api.get('/api/terminal/cwd')
    terminalCwd.value = res.cwd || ''
  } catch (err) {
    notify(`加载目录失败: ${err.message}`, 'error')
  }
}

const saveTerminalCwd = async () => {
  try {
    const data = await api.post('/api/terminal/cwd', { cwd: terminalCwd.value })
    terminalCwd.value = data.cwd || ''
    notify('终端目录已保存', 'success')
  } catch (err) {
    notify(err.message || '目录无效', 'error')
  }
}

onMounted(loadTerminalCwd)
</script>

<template>
  <section class="panel-page">
    <div class="page-header">
      <div>
        <h2>终端设置</h2>
        <p>设置后端终端技能默认工作目录。</p>
      </div>
      <button class="btn-primary" @click="saveTerminalCwd">保存</button>
    </div>

    <div class="card">
      <label class="field">
        <span>工作目录</span>
        <input v-model="terminalCwd" placeholder="/Users/..." />
      </label>
    </div>
  </section>
</template>
