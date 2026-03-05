const BASE_URL = import.meta.env.VITE_API_BASE_URL ?? 'https://api.skreg.ai'

export async function searchPackages({ query = '', category = '', page = 1 } = {}) {
  const params = new URLSearchParams({ page: String(page) })
  if (query) params.set('q', query)
  if (category) params.set('category', category)

  const res = await fetch(`${BASE_URL}/v1/search?${params}`)
  if (!res.ok) throw new Error(`Search failed: ${res.status}`)
  return res.json()
}
