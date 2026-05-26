<script setup>
import { computed, inject, onMounted, ref } from 'vue'

const skills = inject('skills')
const api = inject('apiClient')
const notify = inject('notify', (msg) => window.alert(msg))
const appActions = inject('appActions')

const skillRuntime = ref(null)
const skillCatalog = ref([])
const skillCatalogQuery = ref('')
const skillCatalogLoading = ref(false)
const skillInstallLoading = ref('')
const skillRescanLoading = ref(false)
const openSkillConfigName = ref('')

const installedPackages = computed(() =>
  (skills.value || []).filter((skill) => skill.managed_by === 'clawhub')
)

const statusText = (skill) => {
  if (skill.runtime?.status) return skill.runtime.status
  if (skill.runtime?.executable === false) return 'installed_not_runnable'
  return skill.enabled ? 'enabled' : 'disabled'
}

const toggleSkill = async (skill) => {
  try {
    await api.post('/api/skills/toggle', { name: skill.name, enabled: !skill.enabled })
    await appActions.refreshGlobalData()
  } catch (err) {
    notify(`切换技能失败: ${err.message}`, 'error')
  }
}

const ensureSkillConfig = (skill) => {
  if (!skill.config) skill.config = {}
  if (!skill.config.env) skill.config.env = {}
}

const updateSkillConfig = async (skill) => {
  try {
    ensureSkillConfig(skill)
    await api.patch(`/api/skills/${encodeURIComponent(skill.name)}`, {
      enabled: skill.enabled,
      api_key_ref: skill?.config?.api_key_ref || null,
      env: skill?.config?.env || {},
    })
    await appActions.refreshGlobalData()
    notify(`${skill.name} 配置已保存`, 'success')
  } catch (err) {
    notify(`配置保存失败: ${err.message}`, 'error')
  }
}

const loadSkillRuntime = async () => {
  try {
    skillRuntime.value = await api.get('/api/skills/runtime')
  } catch (err) {
    skillRuntime.value = null
    notify(`加载运行时失败: ${err.message}`, 'error')
  }
}

const loadSkillCatalog = async () => {
  skillCatalogLoading.value = true
  try {
    const trimmed = skillCatalogQuery.value.trim()
    const path = trimmed
      ? `/api/skills/catalog?limit=24&sort=newest&query=${encodeURIComponent(trimmed)}`
      : '/api/skills/catalog?limit=24&sort=newest'
    const data = await api.get(path)
    let items = Array.isArray(data?.items) ? data.items : []
    if (trimmed) {
      const q = trimmed.toLowerCase()
      items = items.filter((item) => [item?.slug, item?.name, item?.title, item?.description, item?.summary]
        .filter(Boolean)
        .join(' ')
        .toLowerCase()
        .includes(q))
    }
    skillCatalog.value = items
  } catch (err) {
    skillCatalog.value = []
    notify(`加载技能市场失败: ${err.message}`, 'error')
  } finally {
    skillCatalogLoading.value = false
  }
}

const refreshSkillArea = async () => {
  await Promise.all([
    loadSkillRuntime(),
    appActions.refreshGlobalData(),
  ])
  await loadSkillCatalog()
}

const installCatalogSkill = async (slug) => {
  if (!slug) return
  skillInstallLoading.value = slug
  try {
    await api.post('/api/skills/install', { slug })
    await refreshSkillArea()
    notify(`技能已安装: ${slug}`, 'success')
  } catch (err) {
    notify(`安装失败: ${err.message}`, 'error')
  } finally {
    skillInstallLoading.value = ''
  }
}

const uninstallSkillPackage = async (skill) => {
  const slug = skill?.origin?.slug || skill?.skill_dir || skill?.name
  if (!slug) return
  skillInstallLoading.value = slug
  try {
    await api.post('/api/skills/uninstall', { slug })
    await refreshSkillArea()
    notify(`技能已卸载: ${slug}`, 'success')
  } catch (err) {
    notify(`卸载失败: ${err.message}`, 'error')
  } finally {
    skillInstallLoading.value = ''
  }
}

const rescanSkills = async () => {
  skillRescanLoading.value = true
  try {
    await api.post('/api/skills/rescan', {})
    await refreshSkillArea()
    notify('技能已重新扫描', 'success')
  } catch (err) {
    notify(`重新扫描失败: ${err.message}`, 'error')
  } finally {
    skillRescanLoading.value = false
  }
}

onMounted(refreshSkillArea)
</script>

<template>
  <section class="panel-page">
    <div class="page-header">
      <div>
        <h2>技能管理</h2>
        <p>统一管理已安装技能、ClawHub 市场和技能配置。</p>
      </div>
      <div class="inline-actions">
        <button class="btn-secondary" :disabled="skillRescanLoading" @click="rescanSkills">
          {{ skillRescanLoading ? '扫描中...' : '重新扫描' }}
        </button>
        <button class="btn-secondary" @click="refreshSkillArea">刷新</button>
      </div>
    </div>

    <div class="card">
      <div class="section-headline">
        <h3>ClawHub 运行时</h3>
        <span v-if="skillRuntime?.available" class="pill success">可用</span>
        <span v-else class="pill danger">不可用</span>
      </div>
      <div class="meta-grid">
        <div><span class="muted">版本</span><strong>{{ skillRuntime?.version || '-' }}</strong></div>
        <div><span class="muted">工作目录</span><strong>{{ skillRuntime?.workdir || '-' }}</strong></div>
        <div><span class="muted">skills 目录</span><strong>{{ skillRuntime?.skills_dir || '-' }}</strong></div>
        <div><span class="muted">已登录</span><strong>{{ skillRuntime?.logged_in === false ? '否' : '是' }}</strong></div>
      </div>
      <p v-if="skillRuntime?.available && skillRuntime?.logged_in === false" class="callout warning">
        ClawHub 未登录，市场可能为空。先在终端执行 `clawhub login`。
      </p>
      <p v-if="skillRuntime?.auth_error" class="callout warning">{{ skillRuntime.auth_error }}</p>
    </div>

    <div class="card">
      <div class="section-headline">
        <h3>已安装技能</h3>
        <span class="muted">{{ (skills || []).length }} 个</span>
      </div>
      <div class="stack-gap">
        <div v-for="skill in skills" :key="skill.name" class="subcard">
          <div class="card-topline">
            <div>
              <strong>{{ skill.name }}</strong>
              <span v-if="skill.risk_level === 'high'" class="pill warning">HIGH RISK</span>
              <span v-if="skill.managed_by === 'clawhub'" class="pill">CLAWHUB</span>
              <span class="pill">{{ statusText(skill) }}</span>
            </div>
            <div class="inline-actions">
              <button class="btn-secondary" @click="toggleSkill(skill)">
                {{ skill.enabled ? '停用' : '启用' }}
              </button>
              <button
                v-if="skill.requires?.includes('api_key_ref')"
                class="btn-secondary"
                @click="ensureSkillConfig(skill); openSkillConfigName = openSkillConfigName === skill.name ? '' : skill.name"
              >
                {{ openSkillConfigName === skill.name ? '收起配置' : '配置' }}
              </button>
              <button v-if="skill.managed_by === 'clawhub'" class="btn-danger" @click="uninstallSkillPackage(skill)">
                {{ skillInstallLoading === (skill.origin?.slug || skill.skill_dir || skill.name) ? '卸载中...' : '卸载' }}
              </button>
            </div>
          </div>
          <div class="meta-grid compact">
            <div><span class="muted">来源</span><strong>{{ skill.install_source || skill.managed_by || '-' }}</strong></div>
            <div><span class="muted">版本</span><strong>{{ skill.version_or_ref || '-' }}</strong></div>
            <div><span class="muted">目录</span><strong>{{ skill.skill_dir || '-' }}</strong></div>
            <div><span class="muted">运行</span><strong>{{ skill.runtime?.execution_mode || '-' }}</strong></div>
          </div>
          <div v-if="openSkillConfigName === skill.name && skill.requires?.includes('api_key_ref')" class="subcard inset">
            <label class="field">
              <span>SecretRef</span>
              <input v-model="skill.config.api_key_ref" placeholder="例如 siliconflow.default" />
            </label>
            <div class="page-actions">
              <button class="btn-primary" @click="updateSkillConfig(skill)">保存配置</button>
            </div>
          </div>
          <p v-if="skill.install_error" class="callout danger">{{ skill.install_error }}</p>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="section-headline">
        <h3>技能市场</h3>
        <span class="muted">{{ skillCatalog.length }} 个结果</span>
      </div>
      <div class="toolbar">
        <input v-model="skillCatalogQuery" placeholder="搜索 skill slug / 名称" @keydown.enter="loadSkillCatalog" />
        <button class="btn-secondary" :disabled="skillCatalogLoading" @click="loadSkillCatalog">
          {{ skillCatalogLoading ? '加载中...' : '搜索' }}
        </button>
      </div>
      <div class="stack-gap">
        <div v-for="item in skillCatalog" :key="item.slug || item.name" class="subcard">
          <div class="card-topline">
            <div>
              <strong>{{ item.title || item.name || item.slug }}</strong>
              <span class="pill">{{ item.slug }}</span>
            </div>
            <button class="btn-primary" @click="installCatalogSkill(item.slug)">
              {{ skillInstallLoading === item.slug ? '安装中...' : '安装' }}
            </button>
          </div>
          <div class="meta-grid compact">
            <div><span class="muted">作者</span><strong>{{ item.owner_handle || item.author || '-' }}</strong></div>
            <div><span class="muted">更新</span><strong>{{ item.updated_at || '-' }}</strong></div>
          </div>
        </div>
        <p v-if="!skillCatalogLoading && skillCatalog.length === 0" class="muted">当前没有可显示的技能条目。</p>
      </div>
    </div>

    <div v-if="installedPackages.length > 0" class="card">
      <div class="section-headline">
        <h3>ClawHub 已安装包</h3>
        <span class="muted">{{ installedPackages.length }} 个</span>
      </div>
      <div class="meta-grid compact">
        <div v-for="skill in installedPackages" :key="skill.name">
          <span class="muted">{{ skill.name }}</span>
          <strong>{{ skill.origin?.slug || skill.skill_dir || skill.name }}</strong>
        </div>
      </div>
    </div>
  </section>
</template>
