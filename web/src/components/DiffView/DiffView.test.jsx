import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import DiffView from './DiffView.jsx'

const diff = {
  from: '1.0.0',
  to: '1.0.1',
  files: [
    {
      path: 'SKILL.md',
      status: 'modified',
      binary: false,
      hunks: [
        {
          old_start: 1, old_lines: 1, new_start: 1, new_lines: 1,
          lines: [
            { kind: 'delete', text: 'old' },
            { kind: 'insert', text: 'new' },
          ],
        },
      ],
    },
    { path: 'logo.png', status: 'added', binary: true, hunks: [] },
  ],
}

describe('DiffView', () => {
  it('renders file headers, statuses, and changed lines', () => {
    render(<DiffView diff={diff} />)
    expect(screen.getByText('SKILL.md')).toBeInTheDocument()
    expect(screen.getByText('-old')).toBeInTheDocument()
    expect(screen.getByText('+new')).toBeInTheDocument()
    expect(screen.getByText(/Binary file/i)).toBeInTheDocument()
  })

  it('shows an empty-state message when there are no file changes', () => {
    render(<DiffView diff={{ from: '1.0.0', to: '1.0.1', files: [] }} />)
    expect(screen.getByText(/No changes/i)).toBeInTheDocument()
  })
})
