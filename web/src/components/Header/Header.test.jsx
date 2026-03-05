import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import Header from './Header.jsx'

describe('Header', () => {
  it('renders the skreg logo text', () => {
    render(<Header theme="dark" onThemeToggle={() => {}} />)
    expect(screen.getByText('skreg')).toBeInTheDocument()
  })

  it('renders context label', () => {
    render(<Header theme="dark" onThemeToggle={() => {}} />)
    expect(screen.getByText('[skreg.ai]')).toBeInTheDocument()
  })

  it('renders breadcrumb', () => {
    render(<Header theme="dark" onThemeToggle={() => {}} />)
    expect(screen.getByText('▸ Packages')).toBeInTheDocument()
  })

  it('renders theme toggle button', () => {
    render(<Header theme="dark" onThemeToggle={() => {}} />)
    expect(screen.getByRole('button', { name: /switch to light mode/i })).toBeInTheDocument()
  })

  it('calls onThemeToggle when toggle clicked', async () => {
    const user = userEvent.setup()
    const onToggle = vi.fn()
    render(<Header theme="dark" onThemeToggle={onToggle} />)

    await user.click(screen.getByRole('button', { name: /switch to light mode/i }))
    expect(onToggle).toHaveBeenCalled()
  })

  it('shows sun icon in dark theme and moon icon in light theme', () => {
    const { rerender } = render(<Header theme="dark" onThemeToggle={() => {}} />)
    expect(screen.getByRole('button').textContent).toContain('☀️')

    rerender(<Header theme="light" onThemeToggle={() => {}} />)
    expect(screen.getByRole('button').textContent).toContain('🌙')
  })
})
