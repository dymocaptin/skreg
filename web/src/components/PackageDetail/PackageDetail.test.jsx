import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import PackageDetail from './PackageDetail.jsx'

vi.mock('../../api.js', () => ({
  previewPackage: vi.fn(),
}))

import { previewPackage } from '../../api.js'

const mockPreviewPackage = previewPackage

const PKG = {
  id: '1',
  namespace: 'acme',
  name: 'my-skill',
  description: 'A skill description',
  category: 'tools',
  latest_version: '1.2.3',
  verification: 'self_signed',
}

const PKG_PUBLISHER = {
  ...PKG,
  verification: 'publisher',
}

const PREVIEW = {
  files: ['SKILL.md', 'manifest.json', 'references/guide.md'],
  skill_md: '# My Skill\n\nContent here.',
  truncated: false,
}

describe('PackageDetail', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('shows loading state in SKILL.md pane initially', () => {
    previewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={PKG} />)
    expect(screen.getAllByText('⠙ Loading…').length).toBeGreaterThan(0)
  })

  it('renders package name in header', () => {
    previewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={PKG} />)
    expect(screen.getByText('my-skill')).toBeInTheDocument()
  })

  it('shows trusted badge when verification is publisher', () => {
    previewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={{ ...PKG, verification: 'publisher' }} />)
    expect(screen.getByText('✓ trusted')).toBeInTheDocument()
  })

  it('does not show trusted badge for self_signed', () => {
    previewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={PKG} />)
    expect(screen.queryByText('✓ trusted')).not.toBeInTheDocument()
  })

  it('renders version in versions pane', () => {
    previewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={PKG} />)
    expect(screen.getByText('▶ 1.2.3')).toBeInTheDocument()
  })

  it('renders install command', () => {
    previewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={PKG} />)
    expect(screen.getByText(/skreg install acme\/my-skill/)).toBeInTheDocument()
  })

  it('renders file list when loaded', async () => {
    previewPackage.mockResolvedValue(PREVIEW)
    render(<PackageDetail pkg={PKG} />)
    await waitFor(() => expect(screen.getByText('SKILL.md')).toBeInTheDocument())
    expect(screen.getByText('manifest.json')).toBeInTheDocument()
    expect(screen.getByText('references/guide.md')).toBeInTheDocument()
  })

  it('renders fileRoot header when loaded', async () => {
    previewPackage.mockResolvedValue(PREVIEW)
    render(<PackageDetail pkg={PKG} />)
    await waitFor(() => expect(screen.getByText('my-skill@1.2.3/')).toBeInTheDocument())
  })

  it('renders skill_md content when loaded', async () => {
    previewPackage.mockResolvedValue(PREVIEW)
    render(<PackageDetail pkg={PKG} />)
    await waitFor(() => expect(screen.getByText(/# My Skill/)).toBeInTheDocument())
  })

  it('shows truncated indicator when preview is truncated', async () => {
    previewPackage.mockResolvedValue({ ...PREVIEW, truncated: true })
    render(<PackageDetail pkg={PKG} />)
    await waitFor(() => expect(screen.getByText('[truncated]')).toBeInTheDocument())
  })

  it('shows error state when preview fetch fails', async () => {
    previewPackage.mockRejectedValue(new Error('Preview failed: 404'))
    render(<PackageDetail pkg={PKG} />)
    await waitFor(() => {
      const errors = screen.getAllByText('Preview failed: 404')
      expect(errors.length).toBeGreaterThan(0)
    })
  })

  it('shows failed state when latest_version is null', () => {
    previewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={{ ...PKG, latest_version: null }} />)
    const errors = screen.getAllByText('No version available')
    expect(errors.length).toBeGreaterThan(0)
  })

  it('does not call previewPackage when latest_version is null', () => {
    previewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={{ ...PKG, latest_version: null }} />)
    expect(previewPackage).not.toHaveBeenCalled()
  })

  it('copies install command on copy button click', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      writable: true,
      configurable: true,
    })
    previewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={PKG} />)
    await user.click(screen.getByRole('button', { name: /copy install command/i }))
    expect(writeText).toHaveBeenCalledWith('skreg install acme/my-skill')
  })

  it('resets copied state when pkg changes', async () => {
    vi.useFakeTimers()
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
      writable: true,
      configurable: true,
    })
    previewPackage.mockReturnValue(new Promise(() => {}))
    const { rerender } = render(<PackageDetail pkg={PKG} />)
    await user.click(screen.getByRole('button', { name: /copy install command/i }))
    expect(screen.getByRole('button', { name: /copy install command/i }).textContent).toContain('copied!')
    rerender(<PackageDetail pkg={{ ...PKG, name: 'other-skill' }} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /copy install command/i }).textContent).not.toContain('copied!')
    )
    vi.useRealTimers()
  })

  it('shows ▶ unknown when latest_version is null', async () => {
    const nullVersionPkg = { ...PKG_PUBLISHER, latest_version: null }
    render(<PackageDetail pkg={nullVersionPkg} />)
    expect(screen.getByText('▶ unknown')).toBeInTheDocument()
  })

  it('does not show [truncated] when truncated is false', async () => {
    previewPackage.mockResolvedValue(PREVIEW)
    render(<PackageDetail pkg={PKG} />)
    await waitFor(() => expect(screen.getByText(/# My Skill/)).toBeInTheDocument())
    expect(screen.queryByText('[truncated]')).not.toBeInTheDocument()
  })

  it('passes AbortController signal to previewPackage', async () => {
    mockPreviewPackage.mockReturnValue(new Promise(() => {}))
    render(<PackageDetail pkg={PKG_PUBLISHER} />)
    await waitFor(() => expect(mockPreviewPackage).toHaveBeenCalled())
    const signal = mockPreviewPackage.mock.calls[0][3]
    expect(signal).toBeInstanceOf(AbortSignal)
  })

  it('shows error state in files pane', async () => {
    mockPreviewPackage.mockRejectedValue(new Error('Preview failed: 500'))
    render(<PackageDetail pkg={PKG_PUBLISHER} />)
    await waitFor(() => {
      const errors = screen.getAllByText('Preview failed: 500')
      expect(errors).toHaveLength(2)
    })
  })
})
