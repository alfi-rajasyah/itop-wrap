import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { TicketDetailView } from '@/backend'
import { disposeMocks, selectTicket, startMock, staticBackend, until } from '@/lib/testing'
import TicketDetail from '../TicketDetail'
import TicketSheets from '../TicketSheets'
import NewTicketForm from '../NewTicketForm'

afterEach(disposeMocks)

const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0])

function renderDetail(scope: 'mine' | 'team' = 'mine') {
  return render(
    <>
      <TicketDetail scope={scope} />
      <TicketSheets scope={scope} />
    </>,
  )
}

describe('TicketDetail: states', () => {
  it('shows the placeholder when nothing is selected (SHELL-7)', async () => {
    startMock()
    renderDetail()
    expect(await screen.findByText('Select a ticket to see its details.')).toBeInTheDocument()
  })

  it('shows the header and strip at once, the body and composer once loaded (DET-4, DET-6, LOG-1)', async () => {
    const base = await loadedView()
    staticBackend({ 'detail:mine': { ...base, body: { type: 'loading' }, composer: null, strip: { ...base.strip, linkedCIs: [] } } })
    const first = renderDetail()
    expect(await screen.findByRole('heading', { name: 'VPN drops every hour' })).toBeInTheDocument()
    expect(screen.getByText('Loading the ticket from iTop')).toBeInTheDocument()
    expect(screen.getByLabelText('Caller: Caller 1')).toBeInTheDocument()
    expect(screen.queryByText('Description')).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: 'Public entry' })).not.toBeInTheDocument()
    first.unmount()
    staticBackend({ 'detail:mine': base })
    renderDetail()
    expect(await screen.findByText('Network access › VPN')).toBeInTheDocument()
    expect(screen.queryByText('Loading the ticket from iTop')).not.toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'Public entry' })).toBeInTheDocument()
  })

  it('renders a failed first load with Retry (DET-5)', async () => {
    const base = await loadedView()
    const { sent } = staticBackend({
      'detail:mine': { ...base, body: { type: 'failed', text: "Couldn't load this ticket. Can't reach itop.example.com." }, composer: null },
      timer: { running: null, saving: false, notice: null },
    })
    renderDetail()
    expect(await screen.findByText("Couldn't load this ticket. Can't reach itop.example.com.")).toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: 'Public entry' })).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(sent).toContainEqual({ model: 'detail:mine', intent: { type: 'retry' } })
  })

  it('renders "No description." and an attachments listing failure (DET-31, DET-34)', async () => {
    const base = await loadedView()
    if (base.body.type !== 'loaded') throw new Error()
    staticBackend({
      'detail:mine': {
        ...base,
        body: { ...base.body, description: null },
        attachments: { type: 'failed', text: "Couldn't list attachments. iTop returned error 100: no." },
      },
    })
    renderDetail()
    expect(await screen.findByText('No description.')).toBeInTheDocument()
    expect(screen.getByText("Couldn't list attachments. iTop returned error 100: no.")).toBeInTheDocument()
  })
})

/** A loaded R-000412 snapshot from the mock, to vary in static renders. */
async function loadedView(): Promise<TicketDetailView> {
  const mock = startMock()
  await selectTicket(mock, 'mine', 'R-000412')
  const view = (await mock.getSnapshot('detail:mine')) as TicketDetailView
  disposeMocks()
  return view
}

describe('TicketDetail: header', () => {
  it('shows ref, class, status, needs-you and the strip (DET-7, DET-25…28)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    expect(await screen.findByRole('button', { name: 'R-000412, copy ref' })).toBeInTheDocument()
    expect(screen.getByRole('img', { name: 'User request' })).toBeInTheDocument()
    expect(screen.getByText('Assigned')).toBeInTheDocument()
    expect(screen.getByLabelText('Caller: Caller 1')).toBeInTheDocument()
    expect(screen.getByLabelText('Organization: Org A')).toBeInTheDocument()
    expect(screen.getByLabelText('Team: Network')).toBeInTheDocument()
    expect(screen.getByRole('img', { name: 'Agent: You' })).toBeInTheDocument()
    expect(screen.getByText('1h 15m spent')).toBeInTheDocument()
  })

  it('copies the ref and shows the Copied chip (DET-10)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.click(await screen.findByRole('button', { name: 'R-000412, copy ref' }))
    expect(spy).toHaveBeenCalledWith('detail:mine', { type: 'copyRef' })
    const chip = await screen.findByText('Copied R-000412')
    // Under the title, like the Swift header stack.
    expect(chip.previousElementSibling?.tagName).toBe('H2')
  })

  it('opens a linked CI in Config items (DET-28, SHELL-11)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.click(await screen.findByRole('button', { name: /vpn-gw-01/ }))
    expect(spy).toHaveBeenCalledWith('shell', { type: 'openCI', ci: { key: '55', name: 'vpn-gw-01', className: 'NetworkDevice' } })
  })

  it('Resolve opens the Resolve sheet (DET-15)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    await userEvent.click(await screen.findByRole('button', { name: 'Resolve ticket' }))
    expect(await screen.findByRole('dialog', { name: 'Resolve R-000412' })).toBeInTheDocument()
  })

  it('Assign to me (plain primary; needs-you is the red pill) assigns an unassigned ticket directly (DET-14, DET-17)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'I-000088')
    const spy = vi.spyOn(mock, 'dispatch')
    const button = await screen.findByRole('button', { name: 'Assign to me' })
    expect(button.className).not.toMatch(/bg-need/)
    expect(screen.getByText('Needs you')).toBeInTheDocument()
    const unassigned = screen.getByRole('img', { name: 'Agent: unassigned' })
    // DET-26: no "Agent" tooltip on an unassigned agent.
    await userEvent.hover(unassigned)
    await new Promise((r) => setTimeout(r, 700))
    expect(screen.queryByRole('tooltip', { name: 'Agent' })).toBeNull()
    await userEvent.click(button)
    expect(spy).toHaveBeenCalledWith('detail:mine', { type: 'assignToMe' })
    expect(await screen.findByRole('button', { name: 'Resolve ticket' })).toBeInTheDocument()
  })

  it('a neutral primary when the ticket does not need you (DET-17)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000405')
    expect((await screen.findByRole('button', { name: 'Resolve ticket' })).className).not.toMatch(/bg-need/)
  })

  it('the More menu has the lifecycle items, copy and web link (DET-19…24)', async () => {
    const mock = startMock()
    const open = vi.spyOn(mock.native, 'openUrl').mockResolvedValue()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    await userEvent.click(await screen.findByRole('button', { name: 'More actions' }))
    const menu = await screen.findByRole('menu', { name: 'More actions' })
    const items = within(menu).getAllByRole('menuitem').map((i) => i.textContent)
    expect(items).toEqual(['Reassign…', 'Set to pending…', 'Edit…⌘E', 'Copy ref R-000412', 'Open in iTop on the web'])
    await userEvent.click(within(menu).getByRole('menuitem', { name: 'Open in iTop on the web' }))
    expect(open).toHaveBeenCalledWith('https://itop.example.com/pages/UI.php?operation=details&class=UserRequest&id=412')
    await userEvent.click(screen.getByRole('button', { name: 'More actions' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Set to pending…' }))
    expect(await screen.findByRole('dialog', { name: 'Set R-000412 to pending' })).toBeInTheDocument()
  })

  it('Close asks first, then closes (DET-16)', async () => {
    const mock = startMock()
    await act(() => mock.dispatch('options', { type: 'setIncludeClosed', includeClosed: true }))
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000355')
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.click(await screen.findByRole('button', { name: 'Close ticket' }))
    const dialog = await screen.findByRole('alertdialog', { name: 'Close R-000355?' })
    expect(within(dialog).getByText("A closed ticket can't be reopened from the app.")).toBeInTheDocument()
    expect(spy).not.toHaveBeenCalledWith('detail:mine', { type: 'close' })
    await userEvent.click(within(dialog).getByRole('button', { name: 'Close ticket' }))
    expect(spy).toHaveBeenCalledWith('detail:mine', { type: 'close' })
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
  })

  it('Close confirmation cancels with Esc and sends nothing', async () => {
    const mock = startMock()
    await act(() => mock.dispatch('options', { type: 'setIncludeClosed', includeClosed: true }))
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000355')
    const spy = vi.spyOn(mock, 'dispatch')
    await userEvent.click(await screen.findByRole('button', { name: 'Close ticket' }))
    const dialog = await screen.findByRole('alertdialog')
    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Close ticket' })).toHaveFocus())
    await userEvent.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    expect(spy).not.toHaveBeenCalledWith('detail:mine', { type: 'close' })
  })

  it('⌘T starts and stops the timer (KEY-4, TMR-19)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    expect(await screen.findByRole('button', { name: 'Start timer' })).toBeInTheDocument()
    await userEvent.keyboard('{Meta>}t{/Meta}')
    expect(await screen.findByRole('button', { name: 'Stop timer' })).toHaveTextContent(/\d\d:\d\d:\d\d/)
    await userEvent.keyboard('{Meta>}t{/Meta}')
    expect(await screen.findByRole('button', { name: 'Start timer' })).toBeInTheDocument()
  })

  it('shows the web stopwatch label instead of a button (TMR-19)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000405')
    expect(await screen.findByText(/^Web stopwatch since /)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Start timer' })).not.toBeInTheDocument()
  })

  it('writes off: primary disabled with its tooltip text, composer read-only (DET-18, LOG-10)', async () => {
    const mock = startMock({ readOnly: true })
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    expect(await screen.findByRole('button', { name: 'Resolve ticket' })).toBeDisabled()
    expect(screen.getByRole('textbox', { name: 'Public entry' })).toHaveAttribute('readonly')
    expect(screen.getByText('Adding entries is switched off in this build, so nothing is sent to iTop.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Add images' })).toBeDisabled()
  })
})

describe('TicketDetail: Edit ticket (iTop Wrap addition)', () => {
  it('Edit… opens the prefilled form; a formatted description is read-only; Save waits for a change', async () => {
    const mock = startMock()
    render(
      <>
        <TicketDetail scope="mine" />
        <NewTicketForm instance="edit" />
      </>,
    )
    await selectTicket(mock, 'mine', 'R-000412')
    await userEvent.click(await screen.findByRole('button', { name: 'More actions' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: /Edit…/ }))
    const dialog = await screen.findByRole('dialog', { name: 'Edit R-000412' })
    await waitFor(() => expect(within(dialog).queryByText('Reading the ticket from iTop…')).toBeNull())
    expect(within(dialog).getByText(/images or formatting/)).toBeTruthy()
    expect(within(dialog).queryByRole('radio', { name: 'Incident' })).toBeNull()
    expect(within(dialog).queryByRole('checkbox', { name: 'Assign to me' })).toBeNull()
    const save = within(dialog).getByRole('button', { name: 'Save changes' }) as HTMLButtonElement
    expect(save.disabled).toBe(true)
    const title = within(dialog).getByRole('textbox', { name: /Title/ })
    await userEvent.type(title, ' now')
    await waitFor(() => expect(save.disabled).toBe(false))
    await userEvent.click(save)
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Edit R-000412' })).toBeNull())
  })
})

describe('TicketDetail: description, attachments and logs', () => {
  it('opens links and outside images through openUrl (DET-43, DET-46)', async () => {
    const mock = startMock()
    const open = vi.spyOn(mock.native, 'openUrl').mockResolvedValue()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    await userEvent.click(await screen.findByRole('link', { name: 'VPN timeout article' }))
    expect(open).toHaveBeenCalledWith(expect.stringMatching(/^https?:/))
    await userEvent.click(screen.getByRole('link', { name: /images\.example\.net ↗/ }))
    expect(open).toHaveBeenCalledWith('https://images.example.net/screenshot.png')
  })

  it('shows an image, opens the viewer and closes it with Esc (DET-47, DET-49)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    expect(await screen.findByText('Loading image')).toBeInTheDocument()
    const button = screen.getByRole('button', { name: 'Image, open full size', hidden: true })
    fireEvent.load(button.querySelector('img')!)
    expect(screen.queryByText('Loading image')).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Image, open full size' }))
    const viewer = await screen.findByRole('dialog', { name: 'Image' })
    expect(within(viewer).getByRole('checkbox', { name: 'Actual size' })).toBeInTheDocument()
    expect(within(viewer).getByRole('button', { name: 'Close' })).toHaveFocus()
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: 'Image' })).not.toBeInTheDocument()
  })

  it('shows the image failure with the web link (DET-47)', async () => {
    const mock = startMock()
    const open = vi.spyOn(mock.native, 'openUrl').mockResolvedValue()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    await screen.findByText('Loading image')
    fireEvent.error(screen.getByRole('button', { name: 'Image, open full size', hidden: true }).querySelector('img')!)
    expect(await screen.findByText("Couldn't load this image.")).toBeInTheDocument()
    await userEvent.click(screen.getByRole('link', { name: 'Open in iTop on the web' }))
    expect(open).toHaveBeenCalledWith('https://itop.example.com/pages/UI.php?operation=details&class=UserRequest&id=412')
  })

  it('downloads an attachment through Rust (DET-33)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    await userEvent.click(await screen.findByRole('button', { name: 'Download vpn-client.log' }))
    expect(await screen.findByText(/^Saved vpn-client\.log \(/)).toBeInTheDocument()
  })

  it('switches between the public and private log (DET-35, DET-36)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    expect(await screen.findByText('The VPN disconnects at the top of every hour and I have to sign in again.')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('radio', { name: 'Private log' }))
    expect(await screen.findByRole('textbox', { name: 'Private entry' })).toBeInTheDocument()
    expect(screen.queryByText('The VPN disconnects at the top of every hour and I have to sign in again.')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Add to private log/ })).toBeInTheDocument()
  })
})

describe('TicketDetail: composer', () => {
  it('types a draft, sends it with ⌘↩ and shows the new entry (LOG-5, LOG-7)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    const box = await screen.findByRole('textbox', { name: 'Public entry' })
    expect(screen.getByRole('button', { name: /Add to public log/ })).toBeDisabled()
    await userEvent.type(box, 'Rebooted the gateway')
    await until(mock, 'detail:mine', (s) => s.type === 'ticket' && s.composer?.draft === 'Rebooted the gateway')
    expect(screen.getByRole('button', { name: /Add to public log/ })).toBeEnabled()
    await userEvent.keyboard('{Meta>}{Enter}{/Meta}')
    expect(await screen.findByText('Rebooted the gateway', { selector: 'p' })).toBeInTheDocument()
    await waitFor(() => expect(box).toHaveValue(''))
  })

  it('keeps the draft and shows the error when the send fails (LOG-8)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    const box = await screen.findByRole('textbox', { name: 'Public entry' })
    await userEvent.type(box, 'please fail')
    await until(mock, 'detail:mine', (s) => s.type === 'ticket' && s.composer?.canSend === true)
    await userEvent.click(screen.getByRole('button', { name: /Add to public log/ }))
    expect(await screen.findByText(/^iTop didn't save the entry\. .* Your draft is kept\.$/)).toBeInTheDocument()
    expect(box).toHaveValue('please fail')
  })

  it('takes a pasted screenshot as an unnamed image, then removes it (LOG-14, LOG-20)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    const spy = vi.spyOn(mock, 'dispatch')
    const box = await screen.findByRole('textbox', { name: 'Public entry' })
    const file = new File([PNG], 'image.png', { type: 'image/png' })
    fireEvent.paste(box, { clipboardData: { files: [file], types: ['Files'], getData: () => '' } })
    await waitFor(() =>
      expect(spy).toHaveBeenCalledWith('detail:mine', { type: 'addImages', items: [{ filename: '', dataBase64: expect.any(String), size: PNG.length }] }),
    )
    const list = await screen.findByRole('list', { name: '1 image to add' })
    expect(within(list).getByRole('img', { name: /Pasted image\.png, / })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Remove Pasted image.png' }))
    await waitFor(() => expect(screen.queryByRole('list', { name: '1 image to add' })).not.toBeInTheDocument())
  })

  it('lets text paste normally (LOG-14)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    const spy = vi.spyOn(mock, 'dispatch')
    const box = await screen.findByRole('textbox', { name: 'Public entry' })
    fireEvent.paste(box, { clipboardData: { files: [], types: ['text/plain'], getData: () => 'hello' } })
    expect(spy).not.toHaveBeenCalledWith('detail:mine', expect.objectContaining({ type: 'addImages' }))
  })

  it('⌘↩ sends from anywhere in the window, but not while a dialog is up (LOG-5, KEY-5)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    const box = await screen.findByRole('textbox', { name: 'Public entry' })
    await userEvent.type(box, 'From anywhere')
    await until(mock, 'detail:mine', (s) => s.type === 'ticket' && s.composer?.canSend === true)
    const spy = vi.spyOn(mock, 'dispatch')
    const modal = document.createElement('div')
    modal.setAttribute('aria-modal', 'true')
    document.body.appendChild(modal)
    fireEvent.keyDown(document.body, { key: 'Enter', metaKey: true })
    expect(spy).not.toHaveBeenCalledWith('detail:mine', { type: 'send' })
    modal.remove()
    box.blur()
    fireEvent.keyDown(document.body, { key: 'Enter', metaKey: true })
    expect(spy).toHaveBeenCalledWith('detail:mine', { type: 'send' })
    expect(await screen.findByText('From anywhere', { selector: 'p' })).toBeInTheDocument()
  })

  it('in the app, paste asks Rust to read the pasteboard and pastes text when it finds no images (LOG-14)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    const spy = vi.spyOn(mock, 'dispatch')
    const exec = vi.fn(() => true)
    Object.defineProperty(document, 'execCommand', { value: exec, configurable: true })
    ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
    try {
      const box = await screen.findByRole('textbox', { name: 'Public entry' })
      fireEvent.paste(box, { clipboardData: { files: [], types: ['text/plain'], getData: () => 'vpn-gw-01.png' } })
      await waitFor(() => expect(spy).toHaveBeenCalledWith('detail:mine', { type: 'pasteFromClipboard' }))
      await waitFor(() => expect(exec).toHaveBeenCalledWith('insertText', false, 'vpn-gw-01.png'))
    } finally {
      delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__
    }
  })

  it('a drag without images neither highlights the box nor is taken (LOG-15)', async () => {
    const mock = startMock()
    renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    const spy = vi.spyOn(mock, 'dispatch')
    const box = await screen.findByRole('textbox', { name: 'Public entry' })
    const card = box.parentElement!
    const text = new File(['x'], 'notes.txt', { type: 'text/plain' })
    const dataTransfer = { types: ['Files'], items: [{ kind: 'file', type: 'text/plain' }], files: [text] }
    fireEvent.dragEnter(card, { dataTransfer })
    expect(card.className).not.toContain('border-2')
    const dropped = fireEvent.drop(card, { dataTransfer })
    expect(dropped).toBe(true)
    expect(spy).not.toHaveBeenCalledWith('detail:mine', expect.objectContaining({ type: 'addImages' }))
    const png = new File([PNG], 'shot.png', { type: 'image/png' })
    const images = { types: ['Files'], items: [{ kind: 'file', type: 'image/png' }], files: [png] }
    fireEvent.dragEnter(card, { dataTransfer: images })
    expect(card.className).toContain('border-2')
    expect(fireEvent.drop(card, { dataTransfer: images })).toBe(false)
    await waitFor(() => expect(spy).toHaveBeenCalledWith('detail:mine', expect.objectContaining({ type: 'addImages' })))
  })

  it('takes picked files and shows Rust’s intake message (LOG-12, LOG-19)', async () => {
    const mock = startMock()
    const { container } = renderDetail()
    await selectTicket(mock, 'mine', 'R-000412')
    await screen.findByRole('textbox', { name: 'Public entry' })
    const input = container.ownerDocument.querySelector<HTMLInputElement>('input[type=file]')!
    await userEvent.upload(input, new File(['hello'], 'notes.txt', { type: 'text/plain' }), { applyAccept: false })
    expect(await screen.findByText("notes.txt isn't an image iTop Wrap can add. Use PNG, JPEG or GIF.")).toBeInTheDocument()
  })
})
