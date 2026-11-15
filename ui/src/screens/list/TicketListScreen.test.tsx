import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import App from '@/App'
import type { ListSnapshot } from '@/backend'
import { backendDispatch, disposeMocks, mountMock, until } from '@/lib/shellTesting'

vi.mock('@/screens/TicketDetail', () => ({ default: ({ scope }: { scope: string }) => <div data-testid={`detail-${scope}`} /> }))
vi.mock('@/screens/TicketSheets', () => ({ default: ({ scope }: { scope: string }) => <div data-testid={`sheets-${scope}`} /> }))
vi.mock('@/screens/NewTicketForm', () => ({ default: () => null }))

afterEach(disposeMocks)

const listbox = (name = 'My tickets') => screen.findByRole('listbox', { name })
const rowOf = async (ref: string) => {
  const box = await listbox()
  return within(box).getByText(ref).closest('[role=option]') as HTMLElement
}
const isLoaded = (s: ListSnapshot) => s.content.type === 'list'

describe('TicketListScreen: states (LIST-44, LIST-60…63, LIST-68)', () => {
  it('first load: "Loading tickets from iTop", no controls, footer "Loading"', async () => {
    mountMock({ listState: 'loading' })
    render(<App />)
    expect(await screen.findByText('Loading tickets from iTop')).toBeInTheDocument()
    expect(screen.getByText('Loading')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^View options/ })).toBeNull()
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeDisabled()
  })

  it('failed first load: the error and Retry, which refreshes', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock({ listState: 'error' })
    render(<App />)
    expect(await screen.findByText("Can't reach itop.example.com. Check your connection, then retry.")).toBeInTheDocument()
    expect(screen.getByText('Not updated yet')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Retry' }))
    expect(sent('list:mine', { type: 'refresh' })).toBe(true)
  })

  it('empty list: the message, with the View control still there (LIST-45)', async () => {
    mountMock({ listState: 'empty' })
    render(<App />)
    expect(await screen.findByText('Nothing waiting on you. New assignments show up here.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'View options: default' })).toBeInTheDocument()
  })

  it('loaded: group headers with whole-group counts, rows, footer count and "Updated just now"', async () => {
    mountMock()
    render(<App />)
    const box = await listbox()
    const groups = within(box).getAllByRole('group')
    expect(groups.map((g) => within(g).getAllByText(/.+/)[0]!.textContent)).toEqual(['Escalated TTR', 'Escalated TTO', 'Assigned', 'Pending'])
    expect(within(groups[2]!).getByText('2')).toBeInTheDocument()
    const vpn = await rowOf('R-000412')
    expect(within(vpn).getByText('VPN drops every hour')).toBeInTheDocument()
    expect(within(vpn).getByLabelText('Priority 1')).toBeInTheDocument()
    expect(within(vpn).getByLabelText('1h 15m spent')).toBeInTheDocument()
    expect(within(await rowOf('R-000367')).getByLabelText('Needs you')).toBeInTheDocument()
    expect(screen.getByText('6 tickets')).toBeInTheDocument()
    expect(screen.getByText('Updated just now')).toBeInTheDocument()
    expect(screen.getByTestId('detail-mine')).toBeInTheDocument()
    expect(screen.getByTestId('sheets-mine')).toBeInTheDocument()
  })
})

describe('TicketListScreen: selection and keyboard (KEY-3, KEY-9, LIST-64)', () => {
  it('click selects; ↑/↓ on the focused list move the selection', async () => {
    const user = userEvent.setup()
    const { sent, mock } = mountMock()
    render(<App />)
    await user.click(within(await rowOf('R-000412')).getByText('VPN drops every hour'))
    expect(sent('list:mine', { type: 'select', ticketId: 'UserRequest::412' })).toBe(true)
    await waitFor(async () => expect(await rowOf('R-000412')).toHaveAttribute('aria-selected', 'true'))
    expect(await listbox()).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    await until(mock, 'list:mine', (s) => s.selection === 'UserRequest::405')
    await user.keyboard('{ArrowUp}{ArrowUp}')
    await until(mock, 'list:mine', (s) => s.selection === 'Incident::088')
  })

  it('⌘R refreshes the list and never reloads the page; Refresh button does too', async () => {
    const user = userEvent.setup()
    const { dispatch, mock } = mountMock()
    render(<App />)
    await listbox()
    let event!: KeyboardEvent
    await act(async () => {
      event = new KeyboardEvent('keydown', { key: 'r', metaKey: true, bubbles: true, cancelable: true })
      window.dispatchEvent(event)
    })
    expect(event.defaultPrevented).toBe(true)
    expect(dispatch.mock.calls.filter(([m, i]) => m === 'list:mine' && i.type === 'refresh')).toHaveLength(1)
    await until(mock, 'list:mine', (s) => !s.refreshing)
    await user.click(screen.getByRole('button', { name: 'Refresh' }))
    expect(dispatch.mock.calls.filter(([m, i]) => m === 'list:mine' && i.type === 'refresh')).toHaveLength(2)
  })

  it('⌘R does nothing while a sheet, dialog or the image viewer is up (LIST-64, KEY-3)', async () => {
    const { dispatch } = mountMock()
    render(<App />)
    await listbox()
    const modal = document.createElement('div')
    modal.setAttribute('aria-modal', 'true')
    document.body.appendChild(modal)
    let event!: KeyboardEvent
    await act(async () => {
      event = new KeyboardEvent('keydown', { key: 'r', metaKey: true, bubbles: true, cancelable: true })
      window.dispatchEvent(event)
    })
    modal.remove()
    expect(event.defaultPrevented).toBe(true)
    expect(dispatch.mock.calls.filter(([m, i]) => m === 'list:mine' && i.type === 'refresh')).toHaveLength(0)
  })
})

describe('TicketListScreen: rows, notices and lookups', () => {
  it('"Assign to me" pill assigns and the list notice can be dismissed (LIST-57, LIST-58, LIST-69)', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock()
    render(<App />)
    const pill = within(await rowOf('I-000088')).getByRole('button', { name: 'Assign I-000088 to me' })
    await user.click(pill)
    expect(sent('list:mine', { type: 'rowAssignToMe', ticketId: 'Incident::088' })).toBe(true)
    const notice = await screen.findByText('I-000088 is assigned to you.')
    await user.click(within(notice.closest('[role=status]') as HTMLElement).getByRole('button', { name: 'Dismiss' }))
    expect(sent('list:mine', { type: 'clearNotice' })).toBe(true)
    await waitFor(() => expect(screen.queryByText('I-000088 is assigned to you.')).toBeNull())
  })

  it('search sends setQuery; no match text; a full ref lookup shows its result or why not (LIST-18, LIST-24…27)', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock()
    render(<App />)
    await listbox()
    const search = screen.getByRole('searchbox', { name: 'Search tickets' })
    expect(search).toHaveAttribute('placeholder', 'R-000412 or VPN')
    await user.type(search, 'zebra')
    expect(sent('list:mine', { type: 'setQuery', query: 'zebra' })).toBe(true)
    expect(await screen.findByText('No tickets in your list match “zebra”.')).toBeInTheDocument()

    await user.clear(search)
    await user.type(search, 'R-000512')
    const section = await screen.findByText('Search result: not in your queue')
    expect(within(section.closest('[role=group]') as HTMLElement).getByText('Teams calls drop on the Finance floor')).toBeInTheDocument()

    await user.clear(search)
    await user.type(search, 'R-999999')
    expect(await screen.findByText("R-999999 doesn't exist, or your iTop account can't see it.")).toBeInTheDocument()
    // Esc clears the search (KEY-9).
    await user.keyboard('{Escape}')
    expect(search).toHaveValue('')
    expect(sent('list:mine', { type: 'setQuery', query: '' })).toBe(true)
  })

  it('takes a query Rust puts in the search (opening a ticket that is not in a list, SHELL-9)', async () => {
    const { mock } = mountMock()
    render(<App />)
    await listbox()
    await backendDispatch(mock, 'shell', { type: 'openTicket', ticket: { ticketId: 'UserRequest::512', ref: 'R-000512' } })
    await waitFor(() => expect(screen.getByRole('searchbox', { name: 'Search tickets' })).toHaveValue('R-000512'))
    expect(await screen.findByText('Search result: not in your queue')).toBeInTheDocument()
  })

  it('a running timer shows its live clock on the row (LIST-56)', async () => {
    const { mock } = mountMock()
    render(<App />)
    await listbox()
    await backendDispatch(mock, 'timer', { type: 'start', ticketId: 'UserRequest::412' })
    const vpn = await rowOf('R-000412')
    await waitFor(() => expect(within(vpn).getByRole('img', { name: 'Timer running' })).toHaveTextContent(/0:0\d/))
  })

  it('a failed time save: the timer notice with Retry and "Discard this time" (TMR-11, TMR-20)', async () => {
    const user = userEvent.setup()
    let now = new Date('2026-11-16T10:00:00')
    const { mock, sent } = mountMock({ timeFail: true, now: () => now })
    render(<App />)
    await listbox()
    await backendDispatch(mock, 'timer', { type: 'start', ticketId: 'UserRequest::412' })
    now = new Date(now.getTime() + 5 * 60_000)
    await backendDispatch(mock, 'timer', { type: 'stop' })
    const text = await screen.findByText(/^Couldn't save 0:05 to R-000412\./)
    const bar = text.closest('[role=alert]') as HTMLElement
    await user.click(within(bar).getByRole('button', { name: 'Retry' }))
    expect(sent('timer', { type: 'retry' })).toBe(true)
    await user.click(within(screen.getByText(/^Couldn't save/).closest('[role=alert]') as HTMLElement).getByRole('button', { name: 'Discard this time' }))
    expect(sent('timer', { type: 'clearNotice' })).toBe(true)
  })

  it('keeps a resolved selected ticket under "Resolved just now" (LIST-30)', async () => {
    const { mock } = mountMock()
    render(<App />)
    await listbox()
    await backendDispatch(mock, 'list:mine', { type: 'select', ticketId: 'UserRequest::412' })
    await backendDispatch(mock, 'resolve:mine', { type: 'open', ticketId: 'UserRequest::412' })
    await backendDispatch(mock, 'resolve:mine', { type: 'setSolution', solution: 'Raised the gateway timeout.' })
    await backendDispatch(mock, 'resolve:mine', { type: 'confirm' })
    const section = await screen.findByText('Resolved just now')
    const row = within(section.closest('[role=group]') as HTMLElement).getByRole('option')
    expect(row).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByText('R-000412 is resolved.')).toBeInTheDocument()
  })
})

describe('TicketListScreen: org filter, View menu and pager (LIST-33…49)', () => {
  it('org filter menu: pick an org, then "All organizations"', async () => {
    const user = userEvent.setup()
    const { sent, mock } = mountMock({ many: true })
    render(<App />)
    await listbox()
    const chip = screen.getByRole('button', { name: 'Filter by organization: all' })
    expect(chip).toHaveTextContent('All organizations')
    await user.click(chip)
    const menu = await screen.findByRole('menu', { name: 'Organizations' })
    expect(within(menu).getByRole('menuitemradio', { name: 'All organizations' })).toHaveAttribute('aria-checked', 'true')
    await user.click(within(menu).getByRole('menuitemradio', { name: 'Org C' }))
    expect(sent('list:mine', { type: 'toggleOrg', name: 'Org C' })).toBe(true)
    expect(await screen.findByRole('button', { name: 'Filter by organization: Org C' })).toHaveTextContent('Org C')
    await until(mock, 'list:mine', (s) => isLoaded(s) && s.orgFilter.selected === 'Org C')
    await user.click(screen.getByRole('button', { name: 'Filter by organization: Org C' }))
    await user.click(within(await screen.findByRole('menu')).getByRole('menuitemradio', { name: 'All organizations' }))
    expect(sent('list:mine', { type: 'clearOrgFilter' })).toBe(true)
  })

  it('View menu: range, closed toggle, sort and page size send their intents; summary and reset', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock()
    render(<App />)
    await listbox()
    const open = async () => {
      await user.click(screen.getByRole('button', { name: /^View options/ }))
      return screen.findByRole('menu', { name: 'View options' })
    }
    let menu = await open()
    for (const t of ['Show tickets opened', 'Sort inside each status group', 'Per page']) expect(within(menu).getByText(t)).toBeInTheDocument()
    expect(within(menu).queryByRole('menuitem', { name: 'Back to default view' })).toBeNull()
    await user.click(within(menu).getByRole('menuitemradio', { name: 'Last 7 days' }))
    expect(sent('options', { type: 'setRange', range: 7 })).toBe(true)
    expect((await screen.findAllByText('Open, last 7 days')).length).toBeGreaterThan(0)

    menu = await open()
    await user.click(within(menu).getByRole('menuitemcheckbox', { name: 'Include resolved and closed' }))
    expect(sent('options', { type: 'setIncludeClosed', includeClosed: true })).toBe(true)
    await user.keyboard('{Escape}')

    menu = await open()
    await user.click(within(menu).getByRole('menuitemradio', { name: 'Opened, newest first' }))
    expect(sent('options', { type: 'setSort', sort: 'newest' })).toBe(true)
    menu = await open()
    await user.click(within(menu).getByRole('menuitemradio', { name: 'All' }))
    expect(sent('options', { type: 'setPageSize', pageSize: null })).toBe(true)
    expect((await screen.findAllByText('All, last 7 days · Opened, newest first · All on one page')).length).toBeGreaterThan(0)

    menu = await open()
    await user.click(within(menu).getByRole('menuitem', { name: 'Back to default view' }))
    expect(sent('options', { type: 'reset' })).toBe(true)
    await waitFor(() => expect(screen.getByRole('button', { name: 'View options: default' })).toBeInTheDocument())
  })

  it('pager: "1–20 of 69", ‹ disabled on page 1, › goes to page 2 (Team tickets)', async () => {
    const user = userEvent.setup()
    const { sent, mock } = mountMock({ many: true })
    render(<App />)
    await listbox()
    await backendDispatch(mock, 'shell', { type: 'select', item: 'teamTickets' })
    await listbox('Team tickets')
    expect(await screen.findByText('1–20 of 69')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Previous page' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: 'Next page' }))
    expect(sent('list:team', { type: 'setPage', page: 2 })).toBe(true)
    expect(await screen.findByText('21–40 of 69')).toBeInTheDocument()
    // Team tickets never has the org filter (LIST-33).
    expect(screen.queryByRole('button', { name: /^Filter by organization/ })).toBeNull()
    fireEvent.keyDown(window, { key: 'r', metaKey: true })
    expect(sent('list:team', { type: 'refresh' })).toBe(true)
  })
})
