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

export function versionsPath(ns, name) {
  return `/v1/packages/${ns}/${name}/versions`
}

export function diffPath(ns, name, from, to) {
  const params = new URLSearchParams()
  if (from) params.set('from', from)
  if (to) params.set('to', to)
  const qs = params.toString()
  return `/v1/packages/${ns}/${name}/diff${qs ? `?${qs}` : ''}`
}

export async function listVersions(ns, name, signal) {
  const res = await fetch(`${BASE_URL}${versionsPath(ns, name)}`, { signal })
  if (!res.ok) throw new Error(`Versions failed: ${res.status}`)
  return res.json()
}

export async function diffPackage(ns, name, from, to, signal) {
  const res = await fetch(`${BASE_URL}${diffPath(ns, name, from, to)}`, { signal })
  if (!res.ok) throw new Error(`Diff failed: ${res.status}`)
  return res.json()
}
