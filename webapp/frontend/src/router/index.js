import { createRouter, createWebHashHistory } from 'vue-router'
import ChatPage from '../pages/ChatPage.vue'
import ModelsPage from '../pages/ModelsPage.vue'
import SkillsPage from '../pages/SkillsPage.vue'
import RoutingPage from '../pages/RoutingPage.vue'
import RuntimePage from '../pages/RuntimePage.vue'
import DiagnosticsPage from '../pages/DiagnosticsPage.vue'
import TerminalPage from '../pages/TerminalPage.vue'
import LarkPage from '../pages/LarkPage.vue'

const routes = [
  { path: '/', redirect: '/chat' },
  { path: '/chat', component: ChatPage },
  { path: '/models', component: ModelsPage },
  { path: '/skills', component: SkillsPage },
  { path: '/routing', component: RoutingPage },
  { path: '/runtime', component: RuntimePage },
  { path: '/diagnostics', component: DiagnosticsPage },
  { path: '/terminal', component: TerminalPage },
  { path: '/lark', component: LarkPage },
]

export default createRouter({
  history: createWebHashHistory(),
  routes,
})
