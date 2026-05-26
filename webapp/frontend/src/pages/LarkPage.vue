<script setup>
import { inject, onMounted, ref } from 'vue'

const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))

const larkAppId = ref('')
const larkAppSecret = ref('')
const larkHasSecret = ref(false)

const loadLarkConfig = async () => {
  try {
    const data = await api.get('/api/lark/config')
    larkAppId.value = data.app_id || ''
    larkHasSecret.value = !!data.has_app_secret
  } catch (err) {
    notify(`加载 Lark 配置失败: ${err.message}`, 'error')
  }
}

const saveLarkConfig = async () => {
  if (!larkAppId.value.trim() || !larkAppSecret.value.trim()) {
    notify('请填写 Lark App ID 和 App Secret', 'error')
    return
  }
  try {
    await api.post('/api/lark/config', {
      app_id: larkAppId.value.trim(),
      app_secret: larkAppSecret.value.trim(),
    })
    larkHasSecret.value = true
    larkAppSecret.value = ''
    notify('Lark 配置已保存，请重启后端生效。', 'success')
  } catch (err) {
    notify(`保存失败: ${err.message}`, 'error')
  }
}

onMounted(loadLarkConfig)
</script>

<template>
  <section class="panel-page">
    <div class="page-header">
      <div>
        <h2>Lark 配置</h2>
        <p>维护 Lark App ID 和 App Secret。</p>
      </div>
      <button class="btn-primary" @click="saveLarkConfig">保存</button>
    </div>

    <div class="card">
      <div class="meta-grid compact">
        <div><span class="muted">Secret 状态</span><strong>{{ larkHasSecret ? '已保存' : '未保存' }}</strong></div>
      </div>
      <div class="form-grid two-col">
        <label class="field">
          <span>App ID</span>
          <input v-model="larkAppId" />
        </label>
        <label class="field">
          <span>App Secret</span>
          <input v-model="larkAppSecret" type="password" placeholder="重新输入以更新" />
        </label>
      </div>
    </div>
  </section>
</template>
