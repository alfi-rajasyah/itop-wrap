import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { disposeMocks, startMock, until } from '@/lib/testing'
import NewTicketForm from '../NewTicketForm'

afterEach(disposeMocks)

async function openSheet(options = {}) {
  const mock = startMock(options)
  render(<NewTicketForm instance="sheet" />)
  await act(() => mock.dispatch('shell', { type: 'newTicket' }))
  const dialog = await screen.findByRole('dialog', { name: 'New ticket' })
  await waitFor(() => expect(within(dialog).getByRole('combobox', { name: 'Organization' })).toBeEnabled())
  return { mock, dialog }
}

async function pick(dialog: HTMLElement, field: string, option: string) {
  const box = within(dialog).getByRole('combobox', { name: field })
  await waitFor(() => expect(box).toBeEnabled())
  await userEvent.click(box)
  await userEvent.click(within(await screen.findByRole('listbox')).getByRole('option', { name: option }))
}

describe('NewTicketForm: sheet', () => {
  it('shows nothing while closed', async () => {
    startMock()
    render(<NewTicketForm instance="sheet" />)
    await act(() => new Promise((r) => setTimeout(r, 10)))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('has the Swift fields, defaults and required marks (NEW-5…12)', async () => {
    const { dialog } = await openSheet()
    expect(within(dialog).getByRole('radio', { name: 'User request' })).toHaveAttribute('aria-checked', 'true')
    expect(within(dialog).getByRole('combobox', { name: 'Caller' })).toBeDisabled()
    expect(within(dialog).getByRole('combobox', { name: 'Caller' })).toHaveAttribute('placeholder', 'Choose an organization first')
    expect(within(dialog).getByRole('combobox', { name: 'Impact' })).toHaveTextContent('A department')
    expect(within(dialog).getByRole('combobox', { name: 'Urgency' })).toHaveTextContent('Low')
    expect(within(dialog).getByRole('textbox', { name: 'Title' })).toHaveAttribute('placeholder', "What's wrong, in a few words")
    expect(within(dialog).getByRole('checkbox', { name: 'Assign to me' })).toBeChecked()
    expect(within(dialog).getAllByText('Required')).toHaveLength(6)
    // initial focus on the first field to fill
    await waitFor(() => expect(within(dialog).getByRole('combobox', { name: 'Organization' })).toHaveFocus())
  })

  it('names the first missing field and marks the empty ones (NEW-14)', async () => {
    const { mock, dialog } = await openSheet()
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.keyboard('{Meta>}{Enter}{/Meta}')
    expect(spy).toHaveBeenCalledWith('newTicket:sheet', { type: 'create' })
    expect(await within(dialog).findByText('Fill in the organization first.')).toBeInTheDocument()
    expect(within(dialog).getAllByText('Required').filter((el) => el.classList.contains('text-danger'))).toHaveLength(6)
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })

  it('searches the organizations, cascades and creates the ticket (NEW-6…9, NEW-19)', async () => {
    const { mock, dialog } = await openSheet()
    const org = within(dialog).getByRole('combobox', { name: 'Organization' })
    await userEvent.click(org)
    await userEvent.type(org, 'org b')
    const options = within(await screen.findByRole('listbox')).getAllByRole('option')
    expect(options.map((o) => o.textContent)).toEqual(['Org B'])
    await userEvent.keyboard('{Enter}')
    await until(mock, 'newTicket:sheet', (s) => s.type === 'open' && s.caller.enabled)
    const callers = await until(mock, 'newTicket:sheet', (s) => s.type === 'open' && s.caller.items.length > 0)
    if (callers.type !== 'open') throw new Error()
    await pick(dialog, 'Caller', callers.caller.items[0]!.label)
    const services = await until(mock, 'newTicket:sheet', (s) => s.type === 'open' && s.service.items.length > 0)
    if (services.type !== 'open') throw new Error()
    await pick(dialog, 'Service', services.service.items[0]!.label)
    const subs = await until(mock, 'newTicket:sheet', (s) => s.type === 'open' && s.subcategory.items.length > 0)
    if (subs.type !== 'open') throw new Error()
    await pick(dialog, 'Service subcategory', subs.subcategory.items[0]!.label)
    await userEvent.type(within(dialog).getByRole('textbox', { name: 'Title' }), 'Keyboard missing keys')
    await userEvent.type(within(dialog).getByRole('textbox', { name: 'Description' }), 'The F and J keys fell off.')
    await until(mock, 'newTicket:sheet', (s) => s.type === 'open' && s.description === 'The F and J keys fell off.')
    await userEvent.click(within(dialog).getByRole('button', { name: 'Create ticket' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    const shell = await until(mock, 'shell', (s) => s.selection === 'myTickets')
    expect(shell.selection).toBe('myTickets')
  }, 20_000)

  it('Cancel keeps a draft; the next plain start restores it with "Clear form" (NEW-21…23)', async () => {
    const { mock, dialog } = await openSheet()
    await userEvent.type(within(dialog).getByRole('textbox', { name: 'Title' }), 'Half-written')
    await until(mock, 'newTicket:sheet', (s) => s.type === 'open' && s.title === 'Half-written')
    expect(within(dialog).queryByRole('button', { name: 'Clear form' })).not.toBeInTheDocument()
    await userEvent.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await act(() => mock.dispatch('shell', { type: 'newTicket' }))
    const again = await screen.findByRole('dialog', { name: 'New ticket' })
    expect(within(again).getByRole('textbox', { name: 'Title' })).toHaveValue('Half-written')
    await userEvent.click(within(again).getByRole('button', { name: 'Clear form' }))
    await waitFor(() => expect(within(again).getByRole('textbox', { name: 'Title' })).toHaveValue(''))
    expect(within(again).queryByRole('button', { name: 'Clear form' })).not.toBeInTheDocument()
  })

  it('shows the CI subtitle when started from a CI (NEW-3)', async () => {
    const mock = startMock()
    render(<NewTicketForm instance="sheet" />)
    await act(() => mock.dispatch('shell', { type: 'newTicketForCI', key: '55', name: 'vpn-gw-01', orgName: 'Org A' }))
    const dialog = await screen.findByRole('dialog', { name: 'New ticket' })
    expect(within(dialog).getByText('On vpn-gw-01')).toBeInTheDocument()
  })

  it('creation switched off: the note and a disabled Create (NEW-13)', async () => {
    const { dialog } = await openSheet({ readOnly: true })
    expect(within(dialog).getByText('Creating tickets is switched off in this build, so nothing is sent to iTop.')).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: 'Create ticket' })).toBeDisabled()
  })

  it('toggles "Assign to me" through the model (NEW-12)', async () => {
    const { mock, dialog } = await openSheet()
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.click(within(dialog).getByRole('checkbox', { name: 'Assign to me' }))
    expect(spy).toHaveBeenCalledWith('newTicket:sheet', { type: 'setAssignToMe', assignToMe: false })
    await waitFor(() => expect(within(dialog).getByRole('checkbox', { name: 'Assign to me' })).not.toBeChecked())
  })
})

describe('NewTicketForm: window', () => {
  it('fills the window while open; Esc cancels (NEW-2)', async () => {
    const mock = startMock()
    render(<NewTicketForm instance="window" />)
    await act(() => mock.dispatch('newTicket:window', { type: 'open' }))
    expect(await screen.findByRole('heading', { name: 'New ticket' })).toBeInTheDocument()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.click(screen.getByRole('textbox', { name: 'Title' }))
    await userEvent.keyboard('{Escape}')
    expect(spy).toHaveBeenCalledWith('newTicket:window', { type: 'cancel' })
    await waitFor(() => expect(screen.queryByRole('heading', { name: 'New ticket' })).not.toBeInTheDocument())
  })
})
