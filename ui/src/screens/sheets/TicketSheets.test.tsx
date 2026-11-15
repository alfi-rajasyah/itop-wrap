import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { disposeMocks, selectTicket, startMock, staticBackend, until } from '@/lib/testing'
import TicketSheets from '../TicketSheets'

afterEach(disposeMocks)

async function openSheet(kind: 'assign' | 'pending' | 'resolve', ref: string, scope: 'mine' | 'team' = 'mine', options = {}) {
  const mock = startMock(options)
  render(<TicketSheets scope={scope} />)
  if (scope === 'team') await act(() => mock.dispatch('shell', { type: 'select', item: 'teamTickets' }))
  const ticket = await selectTicket(mock, scope, ref)
  await act(() => mock.dispatch(`${kind}:${scope}`, { type: 'open', ticketId: ticket.id }))
  return { mock, ticket }
}

describe('TicketSheets', () => {
  it('shows nothing while every sheet is closed', async () => {
    staticBackend({ 'assign:mine': { type: 'closed' }, 'pending:mine': { type: 'closed' }, 'resolve:mine': { type: 'closed' } })
    render(<TicketSheets scope="mine" />)
    await act(() => new Promise((r) => setTimeout(r, 10)))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })
})

describe('Assign sheet (section 6)', () => {
  it('preselects the team, loads agents and reassigns (ASG-6…13)', async () => {
    const { mock } = await openSheet('assign', 'R-000412')
    const dialog = await screen.findByRole('dialog', { name: 'Reassign R-000412' })
    expect(within(dialog).getByText('VPN drops every hour')).toBeInTheDocument()
    expect(within(dialog).getByText('or choose a team and agent')).toBeInTheDocument()
    const team = within(dialog).getByRole('combobox', { name: 'Team' })
    await waitFor(() => expect(team).toHaveTextContent('Network'))
    const confirm = within(dialog).getByRole('button', { name: 'Reassign ticket' })
    expect(confirm).toBeDisabled()
    const agent = within(dialog).getByRole('combobox', { name: 'Agent' })
    await waitFor(() => expect(agent).toBeEnabled())
    // keyboard: open, move, pick (KEY-14)
    agent.focus()
    await userEvent.keyboard('{ArrowDown}')
    expect(await screen.findByRole('listbox')).toBeInTheDocument()
    await userEvent.keyboard('{ArrowDown}{Enter}')
    await waitFor(() => expect(confirm).toBeEnabled())
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.keyboard('{Meta>}{Enter}{/Meta}')
    expect(spy).toHaveBeenCalledWith('assign:mine', { type: 'confirm' })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })

  it('"Assign to me" is disabled with "It\'s already yours" on your own ticket (ASG-7)', async () => {
    await openSheet('assign', 'R-000412')
    const dialog = await screen.findByRole('dialog', { name: 'Reassign R-000412' })
    expect(within(dialog).getByRole('button', { name: 'Assign to me' })).toBeDisabled()
  })

  it('shows the race error in the sheet (ASG-2, ASG-7)', async () => {
    await openSheet('assign', 'R-000436', 'team')
    const dialog = await screen.findByRole('dialog', { name: 'Assign R-000436' })
    await userEvent.click(within(dialog).getByRole('button', { name: 'Assign to me' }))
    expect(await within(dialog).findByText('Agent 3 took R-000436 a moment ago, so it stays with them.')).toBeInTheDocument()
  })

  it('Esc closes an open dropdown first, then the sheet (KEY-6, ASG-12)', async () => {
    const { mock } = await openSheet('assign', 'R-000412')
    const dialog = await screen.findByRole('dialog', { name: 'Reassign R-000412' })
    const team = within(dialog).getByRole('combobox', { name: 'Team' })
    await waitFor(() => expect(team).toBeEnabled())
    team.focus()
    await userEvent.keyboard('{ArrowDown}')
    expect(await screen.findByRole('listbox')).toBeInTheDocument()
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(spy).not.toHaveBeenCalledWith('assign:mine', { type: 'close' })
    await userEvent.keyboard('{Escape}')
    expect(spy).toHaveBeenCalledWith('assign:mine', { type: 'close' })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })
})

describe('Pending sheet (LC-9…12)', () => {
  it('focuses the reason, marks it on an empty confirm, then sets pending', async () => {
    const { mock } = await openSheet('pending', 'R-000412')
    const dialog = await screen.findByRole('dialog', { name: 'Set R-000412 to pending' })
    const reason = within(dialog).getByRole('textbox', { name: 'Reason' })
    expect(reason).toHaveAttribute('placeholder', "What are you waiting for? For example the caller's reply or a part.")
    await waitFor(() => expect(reason).toHaveFocus())
    expect(within(dialog).getByText('Required')).not.toHaveClass('text-danger')
    await userEvent.keyboard('{Meta>}{Enter}{/Meta}')
    expect(await within(dialog).findByText('Enter why the ticket is waiting.')).toBeInTheDocument()
    expect(within(dialog).getByText('Required')).toHaveClass('text-danger')
    await userEvent.type(reason, 'Waiting for the caller')
    await until(mock, 'pending:mine', (s) => s.type === 'open' && s.reason === 'Waiting for the caller')
    await userEvent.click(within(dialog).getByRole('button', { name: 'Set to pending' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    const list = await until(mock, 'list:mine', (s) => s.content.type === 'list' && s.content.body.notice !== null)
    expect(list.content.type === 'list' && list.content.body.notice?.text).toBe('R-000412 is set to pending.')
  })

  it('shows the timer line when the timer runs on the ticket (LC-11)', async () => {
    const mock = startMock()
    render(<TicketSheets scope="mine" />)
    const ticket = await selectTicket(mock, 'mine', 'R-000412')
    await act(() => mock.dispatch('timer', { type: 'start', ticketId: ticket.id }))
    await act(() => mock.dispatch('pending:mine', { type: 'open', ticketId: ticket.id }))
    expect(await screen.findByText(/^Your timer stops and saves \d+:\d\d to this ticket\.$/)).toBeInTheDocument()
  })

  it('keeps the text and shows iTop’s refusal (LC-12)', async () => {
    const { mock } = await openSheet('pending', 'R-000412')
    const dialog = await screen.findByRole('dialog', { name: 'Set R-000412 to pending' })
    await userEvent.type(within(dialog).getByRole('textbox', { name: 'Reason' }), 'this will fail')
    await until(mock, 'pending:mine', (s) => s.type === 'open' && s.reason === 'this will fail')
    await userEvent.click(within(dialog).getByRole('button', { name: 'Set to pending' }))
    expect(await within(dialog).findByText(/^iTop didn't set R-000412 to pending\./)).toBeInTheDocument()
    expect(within(dialog).getByRole('textbox', { name: 'Reason' })).toHaveValue('this will fail')
  })
})

describe('Resolve sheet (LC-13…21)', () => {
  it('prefills service and subcategory, requires the solution, then resolves', async () => {
    const { mock } = await openSheet('resolve', 'R-000412')
    const dialog = await screen.findByRole('dialog', { name: 'Resolve R-000412' })
    await waitFor(() => expect(within(dialog).getByRole('combobox', { name: 'Service' })).toHaveTextContent('Network access'))
    await waitFor(() => expect(within(dialog).getByRole('combobox', { name: 'Service subcategory' })).toHaveTextContent('VPN'))
    expect(within(dialog).getByRole('combobox', { name: 'Resolution code' })).toHaveTextContent('Assistance')
    const solution = within(dialog).getByRole('textbox', { name: 'Solution' })
    await waitFor(() => expect(solution).toHaveFocus())
    await userEvent.click(within(dialog).getByRole('button', { name: 'Resolve ticket' }))
    expect(await within(dialog).findByText('Describe what fixed it. The caller sees this.')).toBeInTheDocument()
    await userEvent.type(solution, 'Raised the session timeout.')
    await until(mock, 'resolve:mine', (s) => s.type === 'open' && s.solution === 'Raised the session timeout.')
    await userEvent.keyboard('{Meta>}{Enter}{/Meta}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })

  it('changing the service clears the subcategory (LC-16)', async () => {
    const { mock } = await openSheet('resolve', 'R-000412')
    const dialog = await screen.findByRole('dialog', { name: 'Resolve R-000412' })
    const service = within(dialog).getByRole('combobox', { name: 'Service' })
    await waitFor(() => expect(within(dialog).getByRole('combobox', { name: 'Service subcategory' })).toHaveTextContent('VPN'))
    await waitFor(() => expect(within(dialog).getByRole('textbox', { name: 'Solution' })).toHaveFocus())
    await userEvent.click(service)
    const options = within(await screen.findByRole('listbox')).getAllByRole('option')
    const other = options.find((o) => o.getAttribute('aria-selected') === 'false')!
    await userEvent.click(other)
    await until(mock, 'resolve:mine', (s) => s.type === 'open' && s.subcategory.selectedId === null)
    await waitFor(() => expect(within(dialog).getByRole('combobox', { name: 'Service subcategory' })).not.toHaveTextContent('VPN'))
  })
})
