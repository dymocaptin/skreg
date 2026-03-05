import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import App from './App.jsx'

describe('App', () => {
  beforeEach(() => {
    localStorage.clear()
    vi.spyOn(global, 'fetch').mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ packages: [], total: 0, page: 1 }),
    })
  })

  afterEach(() => {
    vi.restoreAllMocks()
    document.documentElement.removeAttribute('data-theme')
  })

  it('renders the skreg logo', async () => {
    render(<App />)
    expect(screen.getByText('skreg')).toBeInTheDocument()
  })

  it('defaults to dark theme (no data-theme attribute)', () => {
    render(<App />)
    expect(document.documentElement).not.toHaveAttribute('data-theme', 'light')
  })

  it('toggles to light theme when toggle is clicked', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: /switch to light mode/i }))

    expect(document.documentElement).toHaveAttribute('data-theme', 'light')
  })

  it('persists theme to localStorage', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: /switch to light mode/i }))

    expect(localStorage.getItem('theme')).toBe('light')
  })

  it('restores theme from localStorage on mount', () => {
    localStorage.setItem('theme', 'light')
    render(<App />)
    expect(document.documentElement).toHaveAttribute('data-theme', 'light')
  })

  it('renders PackageGrid table', async () => {
    render(<App />)
    await waitFor(() => {
      expect(document.querySelector('table')).toBeInTheDocument()
    })
  })
})
