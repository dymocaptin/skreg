// In dev the Vite proxy forwards /v1/* to api.skreg.ai, avoiding CORS.
// In production (or when overridden) use the explicit base URL.
const BASE_URL = import.meta.env.VITE_API_BASE_URL ?? (import.meta.env.DEV ? '' : 'https://api.skreg.ai')

export async function searchPackages({ query = '', category = '', page = 1 } = {}) {
  const params = new URLSearchParams({ page: String(page) })
  if (query) params.set('q', query)
  if (category) params.set('category', category)

  const res = await fetch(`${BASE_URL}/v1/search?${params}`)
  if (!res.ok) throw new Error(`Search failed: ${res.status}`)
  return res.json()
}

export async function previewPackage(ns, name, version, signal) {
  const res = await fetch(`${BASE_URL}/v1/packages/${ns}/${name}/${version}/preview`, { signal })
  if (!res.ok) throw new Error(`Preview failed: ${res.status}`)
  return res.json()
}
