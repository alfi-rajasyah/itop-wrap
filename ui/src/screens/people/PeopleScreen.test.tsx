import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { PeopleSnapshot } from '@/backend'
import { disposeMocks, startMock, staticBackend, until } from '@/lib/testing'
import PeopleScreen from '../PeopleScreen'

afterEach(disposeMocks)

describe('PeopleScreen (section 13)', () => {
  it('shows the idle hint and the placeholder (PPL-1, SHELL-7)', async () => {
    startMock()
    render(<PeopleScreen />)
    expect(await screen.findByText('Search by name, for example a caller or an agent.')).toBeInTheDocument()
    expect(screen.getByText('Select a person to see their details and tickets.')).toBeInTheDocument()
  })

  it('searches, opens a person and shows their info and tickets (PPL-2…14)', async () => {
    const mock = startMock()
    render(<PeopleScreen />)
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.type(await screen.findByRole('searchbox', { name: 'Search people by name' }), 'demo')
    expect(spy).toHaveBeenCalledWith('people', { type: 'setQuery', query: 'demo' })
    const list = await screen.findByRole('listbox', { name: 'People' })
    await userEvent.click(within(list).getByText('Demo Agent'))
    await until(mock, 'people', (s) => s.detail?.tickets.type === 'loaded' || s.detail?.tickets.type === 'empty')
    expect(await screen.findByRole('heading', { name: 'Demo Agent' })).toBeInTheDocument()
    expect(screen.getByText('Agent · IT support')).toBeInTheDocument()
    const tab = screen.getByRole('tab', { name: /^Assigned to them/ })
    expect(tab).toHaveAttribute('aria-selected', 'true')
    const table = screen.getByRole('table')
    expect(within(table).getAllByRole('columnheader').map((c) => c.textContent)).toEqual(['Ref', 'Title', 'Caller'])
    await userEvent.click(screen.getByRole('tab', { name: /^Reported by them/ }))
    expect(spy).toHaveBeenCalledWith('people', { type: 'setTab', tab: 'reported' })
  })

  it('links the organization and the email (PPL-9)', async () => {
    const detail: PeopleSnapshot['detail'] = {
      id: '7', displayName: 'Caller One', initials: 'CO', kindLine: 'Caller', organization: { orgId: '1', orgName: 'Org A' },
      email: 'caller.one@example.com', phone: '+00 0000 0001', teams: null, tab: 'reported',
      assignedLabel: 'Assigned to them 0', reportedLabel: 'Reported by them 2', tickets: { type: 'loading', text: 'Loading tickets' },
    }
    const { sent, opened } = staticBackend({
      people: {
        query: 'cal', scope: null, results: { type: 'loaded', banner: null, people: [{ id: '7', displayName: 'Caller One', initials: 'CO', subtitle: 'Caller · Org A' }] },
        selectedId: '7', detail, placeholder: '',
      },
    })
    render(<PeopleScreen />)
    expect(await screen.findByText('Loading tickets')).toBeInTheDocument()
    expect(screen.getByText('+00 0000 0001')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Org A' }))
    expect(sent).toContainEqual({ model: 'shell', intent: { type: 'openOrg', orgId: '1', orgName: 'Org A' } })
    await userEvent.click(screen.getByRole('link', { name: 'caller.one@example.com' }))
    expect(opened).toEqual(['mailto:caller.one@example.com'])
    expect(screen.getByRole('tab', { name: 'Reported by them 2' })).toHaveAttribute('aria-selected', 'true')
  })

  it('shows an org scope with its × and the banner (PPL-3, PPL-5)', async () => {
    const mock = startMock()
    render(<PeopleScreen />)
    await act(() => mock.dispatch('shell', { type: 'openOrgPeople', orgId: '1', orgName: 'Org A' }))
    expect(await screen.findByText('People in Org A')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Clear organization' }))
    await waitFor(() => expect(screen.queryByText('People in Org A')).not.toBeInTheDocument())
  })

  it('renders the searching, failed, empty and banner states (PPL-3, PPL-4)', async () => {
    const base: PeopleSnapshot = { query: 'ab', scope: null, results: { type: 'searching' }, selectedId: null, detail: null, placeholder: '' }
    staticBackend({ people: base })
    let r = render(<PeopleScreen />)
    expect(await screen.findByText('Searching iTop')).toBeInTheDocument()
    r.unmount()
    staticBackend({ people: { ...base, results: { type: 'failed', text: "Couldn't search iTop. Can't reach itop.example.com." } } })
    r = render(<PeopleScreen />)
    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't search iTop.")
    r.unmount()
    staticBackend({ people: { ...base, results: { type: 'empty', text: 'Nobody matches “ab”.' } } })
    r = render(<PeopleScreen />)
    expect(await screen.findByText('Nobody matches “ab”.')).toBeInTheDocument()
    r.unmount()
    const banner = 'Showing 100 of 240 people. Type more of the name to narrow it down.'
    staticBackend({ people: { ...base, results: { type: 'loaded', banner, people: [{ id: '1', displayName: 'Ab C', initials: 'AC', subtitle: 'Caller' }] } } })
    render(<PeopleScreen />)
    expect(await screen.findByText(banner)).toBeInTheDocument()
  })
})
