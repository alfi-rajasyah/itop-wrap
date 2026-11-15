import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import App from '@/App'
import { backendDispatch, disposeMocks, mountMock, until } from '@/lib/shellTesting'

vi.mock('@/screens/TicketDetail', () => ({ default: () => null }))
vi.mock('@/screens/TicketSheets', () => ({ default: () => null }))
vi.mock('@/screens/NewTicketForm', () => ({ default: ({ instance }: { instance: string }) => <div data-testid={`new-ticket-${instance}`} /> }))
vi.mock('@/screens/CmdbScreen', () => ({ default: () => <div data-testid="cmdb" /> }))
vi.mock('@/screens/OrgsScreen', () => ({ default: () => <div data-testid="orgs" /> }))
vi.mock('@/screens/PeopleScreen', () => ({ default: () => <div data-testid="people" /> }))

afterEach(disposeMocks)

const nav = () => screen.findByRole('navigation', { name: 'Sections' })

describe('ShellScreen: sidebar (SHELL-3…5, AUTH-27)', () => {
  it('shows both sections with titles, the Needs-you count and ⌘ hints', async () => {
    mountMock()
    render(<App />)
    const sidebar = await nav()
    expect(within(sidebar).getByText('Tickets')).toBeInTheDocument()
    expect(within(sidebar).getByText('CMDB')).toBeInTheDocument()
    const labels = within(sidebar)
      .getAllByRole('button')
      .map((b) => b.querySelector('.truncate')?.textContent)
    expect(labels).toEqual(['My tickets', 'Team tickets', 'Config items', 'Organizations', 'People'])
    expect(await within(sidebar).findByLabelText('2 need you')).toHaveTextContent('2')
    expect(within(sidebar).getByRole('button', { name: /My tickets/ })).toHaveAttribute('aria-current', 'page')
    expect(within(sidebar).getByRole('button', { name: /Config items/ })).toHaveAttribute('aria-keyshortcuts', 'Meta+3')
    // The content title is the section's (SHELL-5).
    expect(await screen.findByRole('heading', { name: 'My tickets' })).toBeInTheDocument()
    // New ticket can open as a sheet over the shell (SHELL-14).
    expect(screen.getByTestId('new-ticket-sheet')).toBeInTheDocument()
  })

  it('selecting a section sends shell.select and shows it', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock()
    render(<App />)
    const sidebar = await nav()
    await user.click(within(sidebar).getByRole('button', { name: /Team tickets/ }))
    expect(sent('shell', { type: 'select', item: 'teamTickets' })).toBe(true)
    expect(await screen.findByRole('heading', { name: 'Team tickets' })).toBeInTheDocument()
    await user.click(within(sidebar).getByRole('button', { name: /Organizations/ }))
    expect(await screen.findByTestId('orgs')).toBeInTheDocument()
    await user.click(within(sidebar).getByRole('button', { name: /People/ }))
    expect(await screen.findByTestId('people')).toBeInTheDocument()
    await user.click(within(sidebar).getByRole('button', { name: /Config items/ }))
    expect(await screen.findByTestId('cmdb')).toBeInTheDocument()
  })

  it('follows a selection made elsewhere (View menu ⌘1…5 in Rust)', async () => {
    const { mock } = mountMock()
    render(<App />)
    await nav()
    await backendDispatch(mock, 'shell', { type: 'select', item: 'people' })
    expect(await screen.findByTestId('people')).toBeInTheDocument()
  })

  it('⌘1…5 and ⌘N work in the browser mock, where there is no native menu', async () => {
    const { sent } = mountMock()
    render(<App />)
    await nav()
    await act(async () => {
      fireEvent.keyDown(window, { key: '3', metaKey: true })
    })
    expect(sent('shell', { type: 'select', item: 'configItems' })).toBe(true)
    expect(await screen.findByTestId('cmdb')).toBeInTheDocument()
    await act(async () => {
      fireEvent.keyDown(window, { key: 'n', metaKey: true })
    })
    expect(sent('shell', { type: 'newTicket' })).toBe(true)
  })
})

describe('ShellScreen: account and sign out (AUTH-27…30)', () => {
  it('account button: label, tooltip text, and a menu with the disabled identity line and "Sign out…"', async () => {
    const user = userEvent.setup()
    mountMock()
    render(<App />)
    const button = await screen.findByRole('button', { name: 'Account: signed in as Demo Agent on itop.example.com' })
    await user.click(button)
    const menu = await screen.findByRole('menu', { name: 'Account' })
    expect(within(menu).getByRole('menuitem', { name: 'Demo Agent · itop.example.com' })).toBeDisabled()
    expect(within(menu).getByRole('separator')).toBeInTheDocument()
    expect(within(menu).getByRole('menuitem', { name: 'Sign out…' })).toBeEnabled()
  })

  it('asks before signing out; Cancel keeps the session, "Sign out" signs out', async () => {
    const user = userEvent.setup()
    const { sent, mock } = mountMock()
    render(<App />)
    await user.click(await screen.findByRole('button', { name: /^Account:/ }))
    await user.click(await screen.findByRole('menuitem', { name: 'Sign out…' }))
    let dialog = await screen.findByRole('alertdialog', { name: 'Sign out of itop.example.com?' })
    expect(within(dialog).getByText('Your password is removed from the Keychain. Your username stays filled in.')).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull())
    expect(sent('session', { type: 'signOut' })).toBe(false)

    await user.click(screen.getByRole('button', { name: /^Account:/ }))
    await user.click(await screen.findByRole('menuitem', { name: 'Sign out…' }))
    dialog = await screen.findByRole('alertdialog')
    await user.click(within(dialog).getByRole('button', { name: 'Sign out' }))
    expect(sent('session', { type: 'signOut' })).toBe(true)
    expect(await screen.findByRole('heading', { name: 'Sign in to iTop' })).toBeInTheDocument()
    await until(mock, 'session', (s) => s.type === 'signedOut')
  })
})
