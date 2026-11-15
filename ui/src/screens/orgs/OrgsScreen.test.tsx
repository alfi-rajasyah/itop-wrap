import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { disposeMocks, startMock, staticBackend, until } from '@/lib/testing'
import OrgsScreen from '../OrgsScreen'

afterEach(disposeMocks)

async function openOrgs() {
  const mock = startMock()
  render(<OrgsScreen />)
  await act(() => mock.dispatch('shell', { type: 'select', item: 'organizations' }))
  const list = await screen.findByRole('listbox', { name: 'Organizations' })
  return { mock, list }
}

describe('OrgsScreen (section 12)', () => {
  it('lists organizations with open counts and the placeholder (ORG-1, ORG-5)', async () => {
    const { list } = await openOrgs()
    const rows = within(list).getAllByRole('option')
    expect(rows.length).toBeGreaterThan(0)
    expect(within(rows[0]!).getByLabelText(/^\d+ open tickets$/)).toBeInTheDocument()
    expect(screen.getByText('Select an organization to see its open tickets.')).toBeInTheDocument()
  })

  it('filters through the model and shows the no-match text (ORG-3, ORG-4)', async () => {
    const { mock } = await openOrgs()
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.type(screen.getByRole('searchbox', { name: 'Search organizations' }), 'zzz')
    expect(spy).toHaveBeenCalledWith('orgs', { type: 'setQuery', query: 'zzz' })
    expect(await screen.findByText('No organization matches “zzz”.')).toBeInTheDocument()
  })

  it('renders loading and failed + Retry (ORG-4)', async () => {
    const base = { query: '', selectedId: null, detail: null, placeholder: 'Select an organization to see its open tickets.' }
    staticBackend({ orgs: { ...base, list: { type: 'loading' } } })
    const { unmount } = render(<OrgsScreen />)
    expect(await screen.findByText('Loading organizations from iTop')).toBeInTheDocument()
    unmount()
    const { sent } = staticBackend({ orgs: { ...base, list: { type: 'failed', text: "Couldn't load organizations. Can't reach itop.example.com." } } })
    render(<OrgsScreen />)
    expect(await screen.findByText("Couldn't load organizations. Can't reach itop.example.com.")).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(sent).toContainEqual({ model: 'orgs', intent: { type: 'retryList' } })
  })

  it('shows an org: header, links, View menu and the grouped ticket table (ORG-6…11)', async () => {
    const { mock, list } = await openOrgs()
    await userEvent.click(within(list).getByText('Org A'))
    await until(mock, 'orgs', (s) => s.detail?.tickets.type === 'loaded')
    expect(await screen.findByRole('heading', { name: 'Org A' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Open tickets' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /View/ })).toBeInTheDocument()
    const table = screen.getByRole('table')
    expect(within(table).getAllByRole('columnheader').map((c) => c.textContent)).toEqual(['Ref', 'Title', 'Caller', 'Assignee'])

    const spy = vi.spyOn(mock, 'dispatch')
    const row = within(table).getAllByRole('row').find((r) => r.hasAttribute('data-ticket-row'))!
    await userEvent.click(row)
    expect(spy).toHaveBeenCalledWith('shell', expect.objectContaining({ type: 'openTicket' }))
    await userEvent.click(screen.getByRole('button', { name: /^People/ }))
    expect(spy).toHaveBeenCalledWith('shell', expect.objectContaining({ type: 'openOrgPeople', orgName: 'Org A' }))
    await userEvent.click(screen.getByRole('button', { name: /^Config items/ }))
    expect(spy).toHaveBeenCalledWith('shell', expect.objectContaining({ type: 'openOrgConfigItems', orgName: 'Org A' }))
    await userEvent.click(screen.getByRole('button', { name: 'New ticket' }))
    expect(spy).toHaveBeenCalledWith('shell', expect.objectContaining({ type: 'newTicketForOrg', orgName: 'Org A' }))
  })

  it('the View menu changes the shared options and shows the summary (LIST-39…43)', async () => {
    const { mock, list } = await openOrgs()
    await userEvent.click(within(list).getByText('Org A'))
    await until(mock, 'orgs', (s) => s.detail?.tickets.type === 'loaded')
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.click(screen.getByRole('button', { name: /View options/ }))
    const menu = await screen.findByRole('menu', { name: 'View options' })
    await userEvent.click(within(menu).getByRole('menuitemcheckbox', { name: 'Include resolved and closed' }))
    expect(spy).toHaveBeenCalledWith('options', { type: 'setIncludeClosed', includeClosed: true })
    expect(await screen.findByRole('heading', { name: 'Tickets' })).toBeInTheDocument()
    expect(await screen.findByText(/^All tickets, any time/, { selector: 'p' })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: /View options/ }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Back to default view' }))
    expect(spy).toHaveBeenCalledWith('options', { type: 'reset' })
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Open tickets' })).toBeInTheDocument())
  })

  it('renders an empty ticket area and a failed one with Retry (ORG-10)', async () => {
    const detail = {
      id: '9', name: 'Org Z', code: '', headerLine: 'Organization', peopleLabel: 'People', configItemsLabel: 'Config items', ticketsHeading: 'Open tickets',
    }
    const base = { query: '', list: { type: 'loaded' as const, orgs: [{ id: '9', name: 'Org Z', code: '', openCount: 0 }] }, selectedId: '9', placeholder: '' }
    staticBackend({ orgs: { ...base, detail: { ...detail, tickets: { type: 'empty', text: 'Org Z has no open tickets.' } } } })
    const { unmount } = render(<OrgsScreen />)
    expect(await screen.findByText('Org Z has no open tickets.')).toBeInTheDocument()
    unmount()
    const { sent } = staticBackend({
      orgs: { ...base, detail: { ...detail, tickets: { type: 'failed', text: "Couldn't load the tickets. Can't reach itop.example.com.", canRetry: true } } },
    })
    render(<OrgsScreen />)
    expect(await screen.findByText("Couldn't load the tickets. Can't reach itop.example.com.")).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(sent).toContainEqual({ model: 'orgs', intent: { type: 'retryTickets' } })
  })
})
