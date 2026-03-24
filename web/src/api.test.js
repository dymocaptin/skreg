import { describe, it, expect, vi, beforeEach } from 'vitest'
import { searchPackages, previewPackage } from './api.js'

describe('searchPackages', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('fetches with no params', async () => {
    const mockData = { packages: [], total: 0, page: 1 }
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    })

    const result = await searchPackages({})

    expect(fetch).toHaveBeenCalledWith(
      expect.stringMatching(/\/v1\/search\?.*page=1/)
    )
    const calledUrl = fetch.mock.calls[0][0]
    expect(calledUrl).not.toContain('q=')
    expect(calledUrl).not.toContain('category=')
    expect(result).toEqual(mockData)
  })

  it('appends q param when query provided', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ packages: [], total: 0, page: 1 }),
    })

    await searchPackages({ query: 'color' })

    expect(fetch).toHaveBeenCalledWith(expect.stringContaining('q=color'))
  })

  it('appends category param when provided', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ packages: [], total: 0, page: 1 }),
    })

    await searchPackages({ category: 'tools' })

    expect(fetch).toHaveBeenCalledWith(expect.stringContaining('category=tools'))
  })

  it('appends page param', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ packages: [], total: 0, page: 2 }),
    })

    await searchPackages({ page: 2 })

    expect(fetch).toHaveBeenCalledWith(expect.stringContaining('page=2'))
  })

  it('throws on non-ok response', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: false,
      status: 500,
    })

    await expect(searchPackages({})).rejects.toThrow('Search failed: 500')
  })

  it('throws when fetch itself rejects (network error)', async () => {
    vi.spyOn(global, 'fetch').mockRejectedValue(new TypeError('Failed to fetch'))

    await expect(searchPackages({})).rejects.toThrow('Failed to fetch')
  })
})

describe('previewPackage', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('fetches the preview endpoint with correct URL', async () => {
    const mockData = { files: ['SKILL.md'], skill_md: '# Hello', truncated: false }
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    })

    const result = await previewPackage('acme', 'my-skill', '1.2.3', undefined)

    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining('/v1/packages/acme/my-skill/1.2.3/preview'),
      expect.objectContaining({ signal: undefined })
    )
    expect(result).toEqual(mockData)
  })

  it('passes signal to fetch', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ files: [], skill_md: '', truncated: false }),
    })
    const controller = new AbortController()

    await previewPackage('acme', 'my-skill', '1.2.3', controller.signal)

    expect(fetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({ signal: controller.signal })
    )
  })

  it('throws on non-ok response', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({ ok: false, status: 404 })

    await expect(previewPackage('acme', 'my-skill', '1.2.3', undefined))
      .rejects.toThrow('Preview failed: 404')
  })

  it('throws when fetch rejects (network error)', async () => {
    vi.spyOn(global, 'fetch').mockRejectedValue(new TypeError('Failed to fetch'))

    await expect(previewPackage('acme', 'my-skill', '1.2.3', undefined))
      .rejects.toThrow('Failed to fetch')
  })
})
