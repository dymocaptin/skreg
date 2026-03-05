import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import PackageGrid from './PackageGrid.jsx'

const PKGS = [
  { id: '1', namespace: 'ns', name: 'alpha', description: 'desc a', category: 'tools', latest_version: '1.0.0', created_at: '2026-01-01T00:00:00Z' },
  { id: '2', namespace: 'ns', name: 'beta', description: 'desc b', category: 'agents', latest_version: '2.0.0', created_at: '2026-01-02T00:00:00Z' },
]

describe('PackageGrid', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('shows loading state initially', () => {
    vi.spyOn(global, 'fetch').mockReturnValue(new Promise(() => {})) // never resolves
    render(<PackageGrid query="" category="" />)
    expect(screen.getByText(/loading/i)).toBeInTheDocument()
  })

  it('renders package cards after fetch', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ packages: PKGS, total: 2, page: 1 }),
    })

    render(<PackageGrid query="" category="" />)

    await waitFor(() => {
      expect(screen.getByText('ns/alpha')).toBeInTheDocument()
      expect(screen.getByText('ns/beta')).toBeInTheDocument()
    })
  })

  it('shows error message on fetch failure', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({ ok: false, status: 500 })

    render(<PackageGrid query="" category="" />)

    await waitFor(() => {
      expect(screen.getByText(/failed to load/i)).toBeInTheDocument()
    })
  })

  it('shows Load more button when more results exist', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ packages: PKGS, total: 50, page: 1 }),
    })

    render(<PackageGrid query="" category="" />)

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /load more/i })).toBeInTheDocument()
    })
  })

  it('hides Load more when all results shown', async () => {
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ packages: PKGS, total: 2, page: 1 }),
    })

    render(<PackageGrid query="" category="" />)

    await waitFor(() => {
      expect(screen.queryByRole('button', { name: /load more/i })).not.toBeInTheDocument()
    })
  })
})
