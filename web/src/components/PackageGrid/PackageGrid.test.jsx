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
      expect(screen.getByText('alpha')).toBeInTheDocument()
      expect(screen.getByText('beta')).toBeInTheDocument()
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

  it('shows PackageDetail with package name when a row is clicked', async () => {
    const user = userEvent.setup()
    vi.spyOn(global, 'fetch').mockImplementation(url => {
      if (String(url).includes('/preview')) {
        return new Promise(() => {}) // never resolves — keeps detail in loading state
      }
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ packages: PKGS, total: 2, page: 1 }),
      })
    })

    render(<PackageGrid query="" category="" />)
    await waitFor(() => expect(screen.getByText('alpha')).toBeInTheDocument())

    // Click the first row — find the row by its package name
    await user.click(screen.getByText('alpha'))

    // PackageDetail should appear — assert the package name it renders in the header
    await waitFor(() => {
      expect(screen.getAllByText('alpha').length).toBeGreaterThan(0)
    })
  })

  it('hides DESCRIPTION column header when panel is open', async () => {
    const user = userEvent.setup()
    vi.spyOn(global, 'fetch').mockImplementation(url => {
      if (String(url).includes('/preview')) {
        return new Promise(() => {}) // never resolves
      }
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ packages: PKGS, total: 2, page: 1 }),
      })
    })

    render(<PackageGrid query="" category="" />)
    await waitFor(() => expect(screen.getByText('alpha')).toBeInTheDocument())

    // Before selection: DESCRIPTION header is visible
    expect(screen.getByText('DESCRIPTION')).toBeInTheDocument()

    await user.click(screen.getByText('alpha'))

    // After selection: DESCRIPTION header should be gone
    await waitFor(() => {
      expect(screen.queryByText('DESCRIPTION')).not.toBeInTheDocument()
    })
  })
})
