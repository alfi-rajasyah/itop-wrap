import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import App from '@/App'
import { disposeMocks, mountMock } from '@/lib/shellTesting'

// The other agent's screens are not under test here.
vi.mock('@/screens/TicketDetail', () => ({ default: () => null }))
vi.mock('@/screens/TicketSheets', () => ({ default: () => null }))
vi.mock('@/screens/NewTicketForm', () => ({ default: () => null }))

afterEach(disposeMocks)

describe('LoginScreen: sign-in form (AUTH-9…19)', () => {
  it('shows the form with the saved server and user, focus on Password (AUTH-11, AUTH-12)', async () => {
    mountMock({ signedOut: true })
    render(<App />)
    expect(await screen.findByRole('heading', { name: 'Sign in to iTop' })).toBeInTheDocument()
    expect(screen.getByText("Use your iTop account. Your password isn't saved: iTop Wrap signs you out after 12 hours without use, and when it quits.")).toBeInTheDocument()
    expect(screen.getByLabelText('Server URL')).toHaveValue('https://itop.example.com')
    expect(screen.getByLabelText('Server URL')).toHaveAttribute('placeholder', 'https://itop.example.com')
    expect(screen.getByLabelText('Username')).not.toHaveValue('')
    await waitFor(() => expect(screen.getByLabelText('Password')).toHaveFocus())
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
  })

  it('missing fields: says so and focuses the empty username, nothing else sent (AUTH-15)', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock({ signedOut: true })
    render(<App />)
    const username = await screen.findByLabelText('Username')
    await user.clear(username)
    await user.click(screen.getByRole('button', { name: 'Sign in to iTop' }))
    expect(await screen.findByText('Enter your username and password.')).toBeInTheDocument()
    await waitFor(() => expect(username).toHaveFocus())
    expect(sent('session', { type: 'setUser', user: '' })).toBe(true)
  })

  it('invalid credentials: shows the error and moves focus to Password (AUTH-18)', async () => {
    const user = userEvent.setup()
    mountMock({ signedOut: true })
    render(<App />)
    const password = await screen.findByLabelText('Password')
    await user.type(password, 'wrong')
    await user.click(screen.getByLabelText('Server URL'))
    await user.keyboard('{Enter}')
    expect(await screen.findByText('Invalid credentials. Check your username and password.')).toBeInTheDocument()
    await waitFor(() => expect(password).toHaveFocus())
  })

  it('REST refusal: the bordered box with the bold warning (AUTH-17)', async () => {
    const user = userEvent.setup()
    mountMock({ signedOut: true })
    render(<App />)
    await user.type(await screen.findByLabelText('Password'), 'rest{Enter}')
    expect(await screen.findByText("This account can't use the iTop REST API.")).toBeInTheDocument()
    expect(screen.getByText(/Ask your iTop admin to add the "REST Services User" profile to/)).toBeInTheDocument()
  })

  it('warns about http:// and sends each server change (AUTH-13)', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock({ signedOut: true })
    render(<App />)
    const server = await screen.findByLabelText('Server URL')
    await user.clear(server)
    await user.type(server, 'http://itop.local')
    expect(server).toHaveValue('http://itop.local')
    expect(
      await screen.findByText("This address isn't encrypted (http). Your password would be sent in plain text. Use https if your server supports it."),
    ).toBeInTheDocument()
    expect(sent('session', { type: 'setServer', server: 'http://itop.local' })).toBe(true)
  })

  it('shows "Signing in…" while signing in, then the shell (AUTH-16, AUTH-20)', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock({ signedOut: true, delayMs: 40 })
    render(<App />)
    const password = await screen.findByLabelText('Password')
    await user.type(password, 'secret{Enter}')
    expect(await screen.findByRole('button', { name: /Signing in…/ })).toBeDisabled()
    // A second submit while signing in is ignored (AUTH-14).
    fireEvent.submit(password.closest('form')!)
    expect(await screen.findByRole('navigation', { name: 'Sections' })).toBeInTheDocument()
    expect(sent('session', { type: 'signIn', password: 'secret' })).toBe(true)
  })
})

describe('LoginScreen: restore and unreachable (AUTH-1, AUTH-5, KEY-7)', () => {
  it('shows the message, Try again on Return, then "Signing in…" while restoring', async () => {
    const { sent } = mountMock({ unreachable: true, delayMs: 40 })
    render(<App />)
    expect(await screen.findByRole('heading', { name: "Can't reach iTop" })).toBeInTheDocument()
    expect(screen.getByText("Can't reach the server. Check your connection and the server address.")).toBeInTheDocument()
    await act(async () => {
      fireEvent.keyDown(window, { key: 'Enter' })
    })
    expect(sent('session', { type: 'retryRestore' })).toBe(true)
    expect(await screen.findByText('Signing in…')).toBeInTheDocument()
    expect(await screen.findByRole('navigation', { name: 'Sections' })).toBeInTheDocument()
  })

  it('"Sign in with another account" goes to the form', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock({ unreachable: true })
    render(<App />)
    await user.click(await screen.findByRole('button', { name: 'Sign in with another account' }))
    expect(sent('session', { type: 'signInWithAnotherAccount' })).toBe(true)
    expect(await screen.findByRole('heading', { name: 'Sign in to iTop' })).toBeInTheDocument()
  })

  it('"Try again" is a button too', async () => {
    const user = userEvent.setup()
    const { sent } = mountMock({ unreachable: true })
    render(<App />)
    await user.click(await screen.findByRole('button', { name: 'Try again' }))
    expect(sent('session', { type: 'retryRestore' })).toBe(true)
  })
})
