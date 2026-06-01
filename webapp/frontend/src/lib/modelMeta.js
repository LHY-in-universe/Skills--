export const PROVIDER_MAP = {
  siliconflow: 'SiliconFlow',
  deepseek: 'DeepSeek',
  openai: 'OpenAI',
  anthropic: 'Anthropic',
  google: 'Google',
  gemini: 'Google',
  minimax: 'MiniMax',
  mimo: 'MiMo',
  zhipu: 'ZhiPu',
  glm: 'ZhiPu',
  kimi: 'Moonshot',
  'kimi-coding': 'Moonshot',
  moonshot: 'Moonshot',
  qwen: 'Qwen',
  alibaba: 'Qwen',
  dashscope: 'Qwen',
  xai: 'xAI',
  grok: 'xAI',
  nvidia: 'NVIDIA',
  nim: 'NVIDIA',
  'nvidia-nim': 'NVIDIA',
  meta: 'Meta',
  mistral: 'Mistral',
  cohere: 'Cohere',
  baichuan: 'Baichuan',
  yi: 'Yi',
  local: 'Local',
  ollama: 'Local',
  lmstudio: 'Local',
  vllm: 'Local',
  llamacpp: 'Local',
}

export const getProviderFromUrl = (url) => {
  if (!url) return null
  try {
    const host = url.toLowerCase()
    if (host.includes('localhost') || host.includes('127.0.0.1') || host.includes('0.0.0.0')) return 'Local'
    for (const [key, label] of Object.entries(PROVIDER_MAP)) {
      if (host.includes(key)) return label
    }
    const match = url.match(/https?:\/\/([^/:]+)/i)
    if (match && match[1]) {
      const parts = match[1].split('.')
      return parts.length > 1 ? parts[parts.length - 2] : parts[0]
    }
    return 'Other'
  } catch {
    return 'Other'
  }
}

export const getProviderFromModelId = (apiId) => {
  if (!apiId) return 'SiliconFlow'
  const id = apiId.toLowerCase()
  if (id.includes('deepseek')) return 'DeepSeek'
  if (id.includes('claude')) return 'Anthropic'
  if (id.includes('gpt') || id.includes('o1') || id.includes('o3') || id.includes('o4')) return 'OpenAI'
  if (id.includes('gemini') || id.includes('gemma')) return 'Google'
  if (id.includes('grok')) return 'xAI'
  if (id.includes('mimo')) return 'MiMo'
  if (id.includes('minimax') || id.includes('abab')) return 'MiniMax'
  if (id.includes('glm') || id.includes('chatglm') || id.includes('zhipu')) return 'ZhiPu'
  if (id.includes('qwen')) return 'Qwen'
  if (id.includes('moonshot') || id.includes('kimi')) return 'Moonshot'
  if (id.includes('mistral') || id.includes('mixtral') || id.includes('codestral')) return 'Mistral'
  if (id.includes('llama') || id.includes('meta-llama')) return 'Meta'
  if (id.includes('nvidia') || id.includes('nemotron')) return 'NVIDIA'
  if (id.includes('cohere') || id.includes('command')) return 'Cohere'
  if (id.includes('baichuan')) return 'Baichuan'
  if (id.includes('yi-')) return 'Yi'
  if (id.includes('internlm') || id.includes('pro/') || id.includes('zai-org')) return 'SiliconFlow'
  return 'SiliconFlow'
}

export const getProviderName = (model = {}) => {
  if (model.provider) return PROVIDER_MAP[model.provider] || model.provider
  if (model.apiUrl) return getProviderFromUrl(model.apiUrl)
  return getProviderFromModelId(model.apiId)
}
