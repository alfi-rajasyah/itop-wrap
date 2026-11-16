import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import MenuBarApp from '@/MenuBarApp'
import { backendDispatch, disposeMocks, mountMock, until } from '@/lib/shellTesting'

afterEach(disposeMocks)

// Accessible names collapse the two spaces of "Assigned  2".
const section = (name: string) => screen.findByRole('region', { name: name.replace(/\s+/g, ' ') })

describe('MenuBarPanelScreen (MB-7…16, KEY-8)', () => {
  it('lists the attention groups with their counts, no New tickets, and the footer', async () => {
    mountMock()
    render(<MenuBarApp />)
    expect(await screen.findByRole('searchbox', { name: 'Search my tickets' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'New ticket' })).toBeInTheDocument()
    const regions = await screen.findAllByRole('region')
    expect(regions.map((r) => r.getAttribute('aria-label'))).toEqual(['Escalated TTR  1', 'Escalated TTO  1', 'Assigned  2', 'Pending  2'])
    const assigned = await section('Assigned  2')
    expect(within(assigned).getByText('VPN drops every hour')).toBeInTheDocument()
    expect(within(assigned).getByText('1h 15m')).toBeInTheDocument()
    expect(within(assigned).getByRole('button', { name: 'Start timer on R-000412' })).toBeEnabled()
    // Rows that can't start a timer keep an empty slot instead of a button (MB-13).
    expect(within(await section('Pending  2')).queryByRole('button', { name: /timer/ })).toBeNull()
    expect(screen.getByText(/^Updated /)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Open iTop Wrap' })).toBeEnabled()
  })

  it('opens a ticket in the main window', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock()
    render(<MenuBarApp />)
    await user.click(within(await section('Assigned  2')).getByText('VPN drops every hour'))
    expect(sent('menubar', { type: 'openTicket', ticketId: 'UserRequest::412' })).toBe(true)
  })

  it('starts a timer: the ticket moves under "Running" with the live counter and a Stop button', async () => {
    const user = userEvent.setup()
    const { sent, mock } = mountMock()
    render(<MenuBarApp />)
    await user.click(within(await section('Assigned  2')).getByRole('button', { name: 'Start timer on R-000412' }))
    expect(sent('timer', { type: 'start', ticketId: 'UserRequest::412' })).toBe(true)
    const running = await section('Running')
    expect(within(running).getByText('VPN drops every hour')).toBeInTheDocument()
    expect(within(running).getByText(/^00:00:0\d$/)).toBeInTheDocument()
    expect(await section('Assigned  1')).toBeInTheDocument()
    // Another startable row now says it would stop the running one (MB-13).
    await user.click(within(running).getByRole('button', { name: 'Stop timer on R-000412' }))
    expect(sent('timer', { type: 'stop' })).toBe(true)
    await until(mock, 'timer', (t) => t.running === null)
    await waitFor(() => expect(screen.queryByRole('region', { name: 'Running' })).toBeNull())
  })

  it('search filters with its own query, and says when nothing matches', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock()
    render(<MenuBarApp />)
    const search = await screen.findByRole('searchbox', { name: 'Search my tickets' })
    await user.type(search, 'zebra')
    expect(sent('menubar', { type: 'setQuery', query: 'zebra' })).toBe(true)
    expect(await screen.findByText('No tickets of yours match “zebra”.')).toBeInTheDocument()
  })

  it('New ticket, Refresh, Open iTop Wrap and ⌘R/⌘N/⌘O send the panel intents', async () => {
    const user = userEvent.setup()
    const { sent, dispatch, mock } = mountMock()
    render(<MenuBarApp />)
    await section('Assigned  2')
    await user.click(screen.getByRole('button', { name: 'New ticket' }))
    expect(sent('menubar', { type: 'newTicket' })).toBe(true)
    await user.click(screen.getByRole('button', { name: 'Open iTop Wrap' }))
    expect(sent('menubar', { type: 'openMain' })).toBe(true)
    await user.click(screen.getByRole('button', { name: 'Refresh' }))
    expect(sent('menubar', { type: 'refresh' })).toBe(true)
    await until(mock, 'menubar', (m) => !m.refreshing)

    dispatch.mockClear()
    let reload!: KeyboardEvent
    await act(async () => {
      reload = new KeyboardEvent('keydown', { key: 'r', metaKey: true, bubbles: true, cancelable: true })
      window.dispatchEvent(reload)
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'o', metaKey: true, bubbles: true, cancelable: true }))
    })
    expect(reload.defaultPrevented).toBe(true)
    expect(sent('menubar', { type: 'refresh' })).toBe(true)
    expect(sent('menubar', { type: 'openMain' })).toBe(true)
  })

  it('Refresh keeps its icon and is disabled only while reloading; the running row shows the snapshot counter (MB-13, MB-16)', async () => {
    const { mock } = mountMock()
    render(<MenuBarApp />)
    await section('Assigned  2')
    const refresh = screen.getByRole('button', { name: 'Refresh' })
    const icon = refresh.innerHTML
    act(() => void mock.dispatch('menubar', { type: 'refresh' }))
    await waitFor(() => expect(refresh).toBeDisabled())
    expect(refresh.innerHTML).toBe(icon)
    await until(mock, 'menubar', (m) => !m.refreshing)
    await waitFor(() => expect(refresh).toBeEnabled())
    await backendDispatch(mock, 'timer', { type: 'start', ticketId: 'UserRequest::412' })
    const running = await section('Running')
    expect(await within(running).findByText(/^\d\d:\d\d:\d\d$/)).toBeInTheDocument()
  })

  it('loading and failed states (MB-14)', async () => {
    const user = userEvent.setup()
    mountMock({ listState: 'loading' })
    const { unmount } = render(<MenuBarApp />)
    expect(await screen.findByText('Loading tickets from iTop')).toBeInTheDocument()
    unmount()
    disposeMocks()

    const { sent } = mountMock({ listState: 'error' })
    render(<MenuBarApp />)
    expect(await screen.findByText("Couldn't load your tickets. Can't reach itop.example.com. Check your connection, then retry.")).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Retry' }))
    expect(sent('menubar', { type: 'refresh' })).toBe(true)
  })

  it('shows the timer notice with Retry and "Discard this time" (MB-9)', async () => {
    let now = new Date('2026-11-16T10:00:00')
    const { mock } = mountMock({ timeFail: true, now: () => now })
    render(<MenuBarApp />)
    await section('Assigned  2')
    await backendDispatch(mock, 'timer', { type: 'start', ticketId: 'UserRequest::412' })
    now = new Date(now.getTime() + 5 * 60_000)
    await backendDispatch(mock, 'timer', { type: 'stop' })
    const bar = (await screen.findByText(/^Couldn't save 0:05 to R-000412\./)).closest('[role=alert]') as HTMLElement
    expect(within(bar).getByRole('button', { name: 'Retry' })).toBeInTheDocument()
    expect(within(bar).getByRole('button', { name: 'Discard this time' })).toBeInTheDocument()
  })
})
