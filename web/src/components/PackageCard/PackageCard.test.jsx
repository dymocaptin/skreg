import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import PackageCard from './PackageCard.jsx'

const PKG = {
  id: 'abc-123',
  namespace: 'dymocaptin',
  name: 'color-analysis',
  description: 'Analyzes dominant colors in images.',
  category: 'tools',
  latest_version: '1.0.1',
  created_at: '2026-02-20T00:00:00Z',
  verification: 'self_signed',
}

// PackageCard renders a <tr> so it needs a table context
const wrapper = ({ children }) => <table><tbody>{children}</tbody></table>

describe('PackageCard', () => {
  it('renders name and namespace in separate columns', () => {
    render(<PackageCard pkg={PKG} />, { wrapper })
    expect(screen.getByText('color-analysis')).toBeInTheDocument()
    expect(screen.getByText('dymocaptin')).toBeInTheDocument()
  })

  it('renders description', () => {
    render(<PackageCard pkg={PKG} />, { wrapper })
    expect(screen.getByText('Analyzes dominant colors in images.')).toBeInTheDocument()
  })

  it('renders version', () => {
    render(<PackageCard pkg={PKG} />, { wrapper })
    expect(screen.getByText('v1.0.1')).toBeInTheDocument()
  })

  it('copies install command to clipboard on button click', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      writable: true,
      configurable: true,
    })

    render(<PackageCard pkg={PKG} />, { wrapper })
    await user.click(screen.getByRole('button', { name: /copy install command/i }))

    expect(writeText).toHaveBeenCalledWith('skreg install dymocaptin/color-analysis')
  })

  it('does not show Copied! when clipboard write fails', async () => {
    const user = userEvent.setup()
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: vi.fn().mockRejectedValue(new Error('Permission denied')) },
      writable: true,
      configurable: true,
    })

    render(<PackageCard pkg={PKG} />, { wrapper })
    await user.click(screen.getByRole('button', { name: /copy install command/i }))

    expect(screen.queryByText('Copied!')).not.toBeInTheDocument()
  })

  it('shows Copied! text after copy', async () => {
    vi.useFakeTimers()
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
      writable: true,
      configurable: true,
    })

    render(<PackageCard pkg={PKG} />, { wrapper })
    await user.click(screen.getByRole('button', { name: /copy install command/i }))

    expect(screen.getByRole('button', { name: /copy install command/i }).textContent).toContain('Copied!')
    vi.useRealTimers()
  })

  it('hides description cell when hideDesc is true', () => {
    render(<PackageCard pkg={PKG} hideDesc />, { wrapper })
    expect(screen.queryByText('Analyzes dominant colors in images.')).not.toBeInTheDocument()
  })

  it('shows description cell when hideDesc is false (default)', () => {
    render(<PackageCard pkg={PKG} />, { wrapper })
    expect(screen.getByText('Analyzes dominant colors in images.')).toBeInTheDocument()
  })

  it('renders ◈ self for self_signed verification', () => {
    render(<PackageCard pkg={PKG} />, { wrapper })
    expect(screen.getByText('◈ self')).toBeInTheDocument()
  })

  it('renders ✦ pub for publisher verification', () => {
    render(<PackageCard pkg={{ ...PKG, verification: 'publisher' }} />, { wrapper })
    expect(screen.getByText('✦ pub')).toBeInTheDocument()
  })

  it('VERIF column is visible even when hideDesc is true', () => {
    render(<PackageCard pkg={{ ...PKG, verification: 'publisher' }} hideDesc />, { wrapper })
    expect(screen.getByText('✦ pub')).toBeInTheDocument()
  })

  it('renders ◈ self for unknown verification value', () => {
    render(<PackageCard pkg={{ ...PKG, verification: 'unknown_value' }} />, { wrapper })
    expect(screen.getByText('◈ self')).toBeInTheDocument()
  })
})
