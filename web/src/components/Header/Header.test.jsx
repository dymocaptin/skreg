import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import Header from './Header.jsx'

describe('Header', () => {
  it('renders the skreg logo text', () => {
    render(<Header query="" onQueryChange={() => {}} theme="dark" onThemeToggle={() => {}} />)
    expect(screen.getByText('skreg')).toBeInTheDocument()
  })

  it('renders search input with current query', () => {
    render(<Header query="color" onQueryChange={() => {}} theme="dark" onThemeToggle={() => {}} />)
    expect(screen.getByRole('searchbox')).toHaveValue('color')
  })

  it('calls onQueryChange when typing', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    render(<Header query="" onQueryChange={onChange} theme="dark" onThemeToggle={() => {}} />)

    await user.type(screen.getByRole('searchbox'), 'a')
    expect(onChange).toHaveBeenCalledWith('a')
  })

  it('renders theme toggle button', () => {
    render(<Header query="" onQueryChange={() => {}} theme="dark" onThemeToggle={() => {}} />)
    expect(screen.getByRole('button', { name: /switch to light mode/i })).toBeInTheDocument()
  })

  it('calls onThemeToggle when toggle clicked', async () => {
    const user = userEvent.setup()
    const onToggle = vi.fn()
    render(<Header query="" onQueryChange={() => {}} theme="dark" onThemeToggle={onToggle} />)

    await user.click(screen.getByRole('button', { name: /switch to light mode/i }))
    expect(onToggle).toHaveBeenCalled()
  })

  it('shows sun icon in dark theme and moon icon in light theme', () => {
    const { rerender } = render(<Header query="" onQueryChange={() => {}} theme="dark" onThemeToggle={() => {}} />)
    expect(screen.getByRole('button').textContent).toContain('☀️')

    rerender(<Header query="" onQueryChange={() => {}} theme="light" onThemeToggle={() => {}} />)
    expect(screen.getByRole('button').textContent).toContain('🌙')
  })
})
