import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { CMDBSnapshot } from '@/backend'
import { disposeMocks, startMock, staticBackend, until } from '@/lib/testing'
import CmdbScreen from '../CmdbScreen'

afterEach(disposeMocks)

async function searchAndOpen(query: string, name: string) {
  const mock = startMock()
  render(<CmdbScreen />)
  await userEvent.type(await screen.findByRole('searchbox', { name: 'Search config items' }), query)
  const list = await screen.findByRole('listbox', { name: 'Config items' })
  await userEvent.click(within(list).getByText(name))
  await until(mock, 'cmdb', (s) => s.current?.body.type === 'loaded')
  return mock
}

describe('CmdbScreen (section 11)', () => {
  it('shows the idle hint and the detail placeholder (CMDB-1, SHELL-7)', async () => {
    startMock()
    render(<CmdbScreen />)
    expect(await screen.findByText('Search by name, for example a server or application.')).toBeInTheDocument()
    expect(screen.getByText('Select a config item to see its details.')).toBeInTheDocument()
  })

  it('searches through the model and lists results with the header (CMDB-2…5)', async () => {
    const mock = startMock()
    render(<CmdbScreen />)
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.type(await screen.findByRole('searchbox', { name: 'Search config items' }), 'vpn')
    expect(spy).toHaveBeenCalledWith('cmdb', { type: 'setQuery', query: 'vpn' })
    const list = await screen.findByRole('listbox', { name: 'Config items' })
    expect(within(list).getAllByRole('option').length).toBeGreaterThan(0)
    expect(screen.getByText(/^(1 result|\d+ results|First \d+ results)$/)).toBeInTheDocument()
  })

  it('renders the searching, failed and empty states', async () => {
    const base: CMDBSnapshot = { query: 'x', scope: null, results: { type: 'searching' }, selectedId: null, current: null, placeholder: 'Select a config item to see its details.' }
    staticBackend({ cmdb: base })
    const { unmount } = render(<CmdbScreen />)
    expect(await screen.findByText('Searching iTop')).toBeInTheDocument()
    unmount()
    staticBackend({ cmdb: { ...base, results: { type: 'failed', text: "Couldn't search iTop. Can't reach itop.example.com." } } })
    const second = render(<CmdbScreen />)
    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't search iTop. Can't reach itop.example.com.")
    second.unmount()
    staticBackend({ cmdb: { ...base, results: { type: 'empty', text: 'No config items match “zz”.' } } })
    render(<CmdbScreen />)
    expect(await screen.findByText('No config items match “zz”.')).toBeInTheDocument()
  })

  it('shows the CI: header, fields, relationships and open tickets (CMDB-11…15)', async () => {
    const mock = await searchAndOpen('vpn', 'vpn-gw-01')
    expect(await screen.findByRole('heading', { name: 'vpn-gw-01' })).toBeInTheDocument()
    expect(screen.getByText('Relationships')).toBeInTheDocument()
    expect(screen.getByText('Depends on')).toBeInTheDocument()
    expect(screen.getByText(/^(Nothing in the CMDB depends on vpn-gw-01\.|If vpn-gw-01 fails, .* (is|are) affected\.)$/)).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: /Open tickets on this CI/ })).toBeInTheDocument()

    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.click(screen.getByRole('button', { name: /R-000412/ }))
    expect(spy).toHaveBeenCalledWith('shell', { type: 'openTicket', ticket: { ticketId: 'UserRequest::412', ref: 'R-000412' } })

    await userEvent.click(screen.getByRole('button', { name: 'New ticket' }))
    expect(spy).toHaveBeenCalledWith('shell', expect.objectContaining({ type: 'newTicketForCI', key: '55', name: 'vpn-gw-01' }))

    await userEvent.click(screen.getByRole('button', { name: 'Copy name' }))
    const chip = await screen.findByText('Copied vpn-gw-01')
    expect(chip.previousElementSibling?.tagName).toBe('H2')
  })

  it('a ticket row opens that ticket in My tickets, detail included (SHELL-9)', async () => {
    const mock = startMock()
    await until(mock, 'list:mine', (s) => s.content.type === 'list')
    const first = await until(mock, 'list:mine', (s) => s.content.type === 'list')
    if (first.content.type !== 'list') throw new Error()
    const other = first.content.body.sections[0]!.rows[0]!.ticket
    await act(() => mock.dispatch('list:mine', { type: 'select', ticketId: other.id }))
    render(<CmdbScreen />)
    await userEvent.type(await screen.findByRole('searchbox', { name: 'Search config items' }), 'vpn')
    await userEvent.click(within(await screen.findByRole('listbox', { name: 'Config items' })).getByText('vpn-gw-01'))
    await userEvent.click(await screen.findByRole('button', { name: /R-000412/ }))
    const detail = await until(mock, 'detail:mine', (s) => s.type === 'ticket' && s.ticket.ref === 'R-000412')
    expect(detail.type).toBe('ticket')
  })

  it('opens a relationship chip and the web page (CMDB-11, CMDB-14)', async () => {
    const mock = await searchAndOpen('vpn', 'vpn-gw-01')
    const open = vi.spyOn(mock.native, 'openUrl').mockResolvedValue()
    await userEvent.click(await screen.findByRole('button', { name: 'Open in iTop on the web' }))
    expect(open).toHaveBeenCalledWith(expect.stringContaining('/pages/UI.php?operation=details&class=NetworkDevice&id=55'))
    const snap = await mock.getSnapshot('cmdb')
    const body = snap?.current?.body
    const chip = body?.type === 'loaded' ? [...body.dependsOn, ...body.usedBy][0] : undefined
    if (!chip) return
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.click(screen.getByRole('button', { name: new RegExp(chip.name) }))
    expect(spy).toHaveBeenCalledWith('cmdb', { type: 'openCI', key: chip.key, className: chip.className, name: chip.name })
    await waitFor(() => expect(screen.getByRole('heading', { name: chip.name })).toBeInTheDocument())
  })

  it('shows an org scope with its × (CMDB-7)', async () => {
    const mock = startMock()
    render(<CmdbScreen />)
    await act(() => mock.dispatch('shell', { type: 'openOrgConfigItems', orgId: '1', orgName: 'Org A' }))
    expect(await screen.findByText('Config items in Org A')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Clear organization' }))
    await waitFor(() => expect(screen.queryByText('Config items in Org A')).not.toBeInTheDocument())
    expect(screen.getByText('Search by name, for example a server or application.')).toBeInTheDocument()
  })

  it('renders a failed detail with Retry (CMDB-10)', async () => {
    const { sent } = staticBackend({
      cmdb: {
        query: '',
        scope: null,
        results: { type: 'idle', text: 'Search by name, for example a server or application.' },
        selectedId: 'NetworkDevice::55',
        current: {
          id: 'NetworkDevice::55', key: '55', className: 'NetworkDevice', name: 'vpn-gw-01', orgName: 'Org A',
          headerLine: 'Network device', copied: false, webUrl: 'https://itop.example.com/x',
          body: { type: 'failed', text: "Couldn't load this config item. Can't reach itop.example.com." },
        },
        placeholder: 'Select a config item to see its details.',
      },
    })
    render(<CmdbScreen />)
    expect(await screen.findByText("Couldn't load this config item. Can't reach itop.example.com.")).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(sent).toContainEqual({ model: 'cmdb', intent: { type: 'retryDetail' } })
  })
})
