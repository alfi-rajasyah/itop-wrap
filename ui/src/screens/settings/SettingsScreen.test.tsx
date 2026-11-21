import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import SettingsScreen from '@/screens/SettingsScreen'
import { disposeMocks, startMock } from '@/lib/testing'

afterEach(disposeMocks)

describe('SettingsScreen (app icons)', () => {
  it('shows both icons on, and keeps one on when the other is hidden', async () => {
    const user = userEvent.setup()
    startMock({ signedOut: true })
    render(<SettingsScreen />)
    const dock = await screen.findByRole('checkbox', { name: 'Show in the Dock' })
    const menuBar = screen.getByRole('checkbox', { name: 'Show in the menu bar' })
    expect(dock).toBeChecked()
    expect(menuBar).toBeChecked()
    expect(menuBar).toBeEnabled()

    await user.click(dock)
    expect(await screen.findByText(/One of the two stays on/)).toBeInTheDocument()
    expect(dock).not.toBeChecked()
    expect(menuBar).toBeChecked()
    expect(menuBar).toBeDisabled()

    await user.click(dock)
    expect(dock).toBeChecked()
    expect(menuBar).toBeEnabled()
    await user.click(menuBar)
    expect(menuBar).not.toBeChecked()
    expect(dock).toBeDisabled()
  })
})
