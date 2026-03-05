import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import CategoryFilter from './CategoryFilter.jsx'

const CATEGORIES = ['agents', 'tools', 'formatters']

describe('CategoryFilter', () => {
  it('renders All pill', () => {
    render(<CategoryFilter categories={CATEGORIES} active="" onChange={() => {}} />)
    expect(screen.getByRole('button', { name: 'All' })).toBeInTheDocument()
  })

  it('renders a pill for each category', () => {
    render(<CategoryFilter categories={CATEGORIES} active="" onChange={() => {}} />)
    for (const cat of CATEGORIES) {
      expect(screen.getByRole('button', { name: cat })).toBeInTheDocument()
    }
  })

  it('marks the active pill with aria-pressed=true', () => {
    render(<CategoryFilter categories={CATEGORIES} active="tools" onChange={() => {}} />)
    expect(screen.getByRole('button', { name: 'tools' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: 'All' })).toHaveAttribute('aria-pressed', 'false')
  })

  it('calls onChange with empty string when All is clicked', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    render(<CategoryFilter categories={CATEGORIES} active="tools" onChange={onChange} />)

    await user.click(screen.getByRole('button', { name: 'All' }))
    expect(onChange).toHaveBeenCalledWith('')
  })

  it('calls onChange with category slug when pill clicked', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    render(<CategoryFilter categories={CATEGORIES} active="" onChange={onChange} />)

    await user.click(screen.getByRole('button', { name: 'agents' }))
    expect(onChange).toHaveBeenCalledWith('agents')
  })
})
