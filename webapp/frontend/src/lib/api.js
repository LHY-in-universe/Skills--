export const createApiClient = (base = '') => {
  const parseError = async (res) => {
    const contentType = res.headers.get('content-type') || ''
    const payload = contentType.includes('application/json') ? await res.json() : await res.text()
    const detail = payload?.detail
    const message = typeof detail === 'string'
      ? detail
      : (detail?.message || payload?.message || payload?.error?.message || `HTTP ${res.status}`)
    const err = new Error(message)
    err.status = res.status
    err.payload = payload
    throw err
  }

  const request = async (path, options = {}) => {
    const res = await fetch(`${base}${path}`, options)
    if (!res.ok) {
      await parseError(res)
    }
    const contentType = res.headers.get('content-type') || ''
    const payload = contentType.includes('application/json') ? await res.json() : await res.text()
    return payload
  }

  return {
    get: (path) => request(path),
    post: (path, body) =>
      request(path, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body ?? {}),
      }),
    patch: (path, body) =>
      request(path, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body ?? {}),
      }),
    del: (path) =>
      request(path, {
        method: 'DELETE',
      }),
    stream: async (path, options = {}) => {
      const res = await fetch(`${base}${path}`, options)
      if (!res.ok) await parseError(res)
      return res
    },
  }
}
