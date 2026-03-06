import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import PackageDetail from './PackageDetail.jsx'

const PKG = {
  id: '1',
  namespace: 'acme',
  name: 'my-skill',
  description: 'A very long description that would normally be truncated in the table view.',
  category: 'tools',
  latest_version: '1.2.3',
}

describe('PackageDetail', () => {
  it('renders package name', () => {
    render(<PackageDetail pkg={PKG} />)
    expect(screen.getByText('my-skill')).toBeInTheDocument()
  })

  it('renders namespace/name@version', () => {
    render(<PackageDetail pkg={PKG} />)
    expect(screen.getByText('acme/my-skill@1.2.3')).toBeInTheDocument()
  })

  it('renders full description', () => {
    render(<PackageDetail pkg={PKG} />)
    expect(screen.getByText(PKG.description)).toBeInTheDocument()
  })

  it('renders category when present', () => {
    render(<PackageDetail pkg={PKG} />)
    expect(screen.getByText('tools')).toBeInTheDocument()
  })

  it('does not render category when absent', () => {
    render(<PackageDetail pkg={{ ...PKG, category: null }} />)
    expect(screen.queryByText('tools')).not.toBeInTheDocument()
  })

  it('renders the install command', () => {
    render(<PackageDetail pkg={PKG} />)
    expect(screen.getByText(/skreg install acme\/my-skill/)).toBeInTheDocument()
  })

  it('copies install command on copy button click', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      writable: true,
      configurable: true,
    })
    render(<PackageDetail pkg={PKG} />)
    await user.click(screen.getByRole('button', { name: /copy install command/i }))
    expect(writeText).toHaveBeenCalledWith('skreg install acme/my-skill')
  })

  it('shows copied! after successful copy', async () => {
    vi.useFakeTimers()
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
      writable: true,
      configurable: true,
    })
    render(<PackageDetail pkg={PKG} />)
    await user.click(screen.getByRole('button', { name: /copy install command/i }))
    expect(screen.getByRole('button', { name: /copy install command/i }).textContent).toContain('copied!')
    vi.useRealTimers()
  })
})
