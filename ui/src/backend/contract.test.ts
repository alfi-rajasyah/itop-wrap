import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import {
  MODEL_IDS,
  WORKSPACE_MODEL_IDS,
  type DetailSnapshot,
  type ListSnapshot,
  type ModelId,
  type SnapshotOf,
  type TicketDetailView,
} from './contract'
import { BackendError, fileToIntake, ModelStore, setStoreForTests, useModel } from './index'
import { MockBackend, mockOptionsFromSearch, type MockOptions } from './mock'

const backends: MockBackend[] = []

function makeMock(options: Partial<MockOptions> = {}) {
  const mock = new MockBackend({ delayMs: 0, ...options })
  backends.push(mock)
  return mock
}

afterEach(() => {
  backends.splice(0).forEach((b) => b.dispose())
})

/** Polls the model until the predicate holds (background loads run after dispatch returns). */
async function until<K extends ModelId>(
  mock: MockBackend,
  model: K,
  predicate: (s: SnapshotOf<K>) => boolean,
): Promise<SnapshotOf<K>> {
  for (let i = 0; i < 200; i++) {
    const s = await mock.getSnapshot(model)
    if (s !== null && predicate(s)) return s
    await new Promise((r) => setTimeout(r, 2))
  }
  throw new Error(`${model} never reached the expected state: ${JSON.stringify(await mock.getSnapshot(model))}`)
}

/**
 * What serde can produce: plain JSON with no `undefined` (missing vs null), no Dates, no
 * functions, every tagged union carrying a string `type`.
 */
function expectSerdeShape(value: unknown, path = '$'): void {
  if (value === null) return
  const kind = typeof value
  if (kind === 'string' || kind === 'boolean') return
  if (kind === 'number') {
    expect(Number.isFinite(value), `${path} is a finite number`).toBe(true)
    return
  }
  expect(kind, `${path} has a JSON type`).toBe('object')
  if (Array.isArray(value)) {
    value.forEach((v, i) => expectSerdeShape(v, `${path}[${i}]`))
    return
  }
  expect(Object.getPrototypeOf(value), `${path} is a plain object`).toBe(Object.prototype)
  for (const [key, v] of Object.entries(value as Record<string, unknown>)) {
    expect(v, `${path}.${key} is not undefined`).not.toBeUndefined()
    if (key === 'type') expect(typeof v, `${path}.type is a string tag`).toBe('string')
    expectSerdeShape(v, `${path}.${key}`)
  }
}

function asTicket(s: DetailSnapshot): TicketDetailView {
  if (s.type !== 'ticket') throw new Error('no ticket in the detail')
  return s
}

function rows(list: ListSnapshot) {
  return list.content.type === 'list' ? list.content.body.sections.flatMap((s) => s.rows) : []
}

describe('contract shape', () => {
  it('lists every model id once, and every workspace model but session', () => {
    expect(new Set(MODEL_IDS).size).toBe(MODEL_IDS.length)
    expect(MODEL_IDS).toHaveLength(23)
    expect(WORKSPACE_MODEL_IDS).not.toContain('session')
    expect(WORKSPACE_MODEL_IDS).toHaveLength(22)
  })

  it('gives serde-shaped snapshots for every model, signed in and signed out', async () => {
    const mock = makeMock()
    await until(mock, 'list:mine', (s) => s.content.type === 'list')
    for (const id of MODEL_IDS) {
      const s = await mock.getSnapshot(id)
      expect(s, `${id} exists while signed in`).not.toBeNull()
      expectSerdeShape(s, id)
    }
    const out = makeMock({ signedOut: true })
    expectSerdeShape(await out.getSnapshot('session'), 'session')
    for (const id of WORKSPACE_MODEL_IDS) expect(await out.getSnapshot(id), `${id} is null signed out`).toBeNull()
  })

  it('keeps the shape through the open states of sheets and forms', async () => {
    const mock = makeMock()
    const list = await until(mock, 'list:mine', (s) => s.content.type === 'list')
    const vpn = rows(list).find((r) => r.ticket.ref === 'R-000412')!.ticket
    await mock.dispatch('list:mine', { type: 'select', ticketId: vpn.id })
    await mock.dispatch('assign:mine', { type: 'open', ticketId: vpn.id })
    await mock.dispatch('pending:mine', { type: 'open', ticketId: vpn.id })
    await mock.dispatch('resolve:mine', { type: 'open', ticketId: vpn.id })
    await mock.dispatch('shell', { type: 'newTicketForOrg', orgId: '1', orgName: 'Org A' })
    await mock.dispatch('cmdb', { type: 'openCI', key: '70', className: 'ApplicationSolution', name: 'Remote access VPN' })
    await mock.dispatch('orgs', { type: 'select', id: '1' })
    for (const id of ['detail:mine', 'assign:mine', 'pending:mine', 'resolve:mine', 'newTicket:sheet', 'cmdb', 'orgs'] as const) {
      const s = await mock.getSnapshot(id)
      expect(s && 'type' in s ? s.type : 'ok').not.toBe('closed')
      expectSerdeShape(s, id)
    }
  })

  it('rejects workspace intents while signed out with an AppError', async () => {
    const mock = makeMock({ signedOut: true })
    const error = await mock.dispatch('list:mine', { type: 'refresh' }).catch((e: unknown) => e)
    expect(error).toBeInstanceOf(BackendError)
    expect((error as BackendError).error.type).toBe('notSignedIn')
  })

  it('reads the Swift demo switches from the URL', () => {
    expect(mockOptionsFromSearch('?state=error&signedOut=1&timeFail=1&delay=5')).toMatchObject({
      listState: 'error',
      signedOut: true,
      timeFail: true,
      delayMs: 5,
    })
    expect(mockOptionsFromSearch('').listState).toBe('normal')
  })
})

describe('main flows', () => {
  it('signs in, loads the list, selects a ticket, and adds a log entry', async () => {
    const mock = makeMock({ signedOut: true })

    const missing = await mock.dispatch('session', { type: 'signIn', password: '' })
    expect(missing).toEqual({ ok: false, focus: 'password' })
    const form = await mock.getSnapshot('session')
    expect(form?.type === 'signedOut' && form.form.message).toEqual({ type: 'missingFields', text: 'Enter your username and password.' })

    const ok = await mock.dispatch('session', { type: 'signIn', password: 'secret' })
    expect(ok).toEqual({ ok: true, focus: null })
    expect((await mock.getSnapshot('session'))?.type).toBe('signedIn')

    const list = await until(mock, 'list:mine', (s) => s.content.type === 'list')
    expect(list.needCount).toBe(2)
    const groups = list.content.type === 'list' ? list.content.body.sections.map((s) => s.title) : []
    expect(groups).toEqual(['Escalated TTR', 'Escalated TTO', 'Assigned', 'Pending'])
    expect((await mock.getSnapshot('shell'))?.sidebar[0]).toEqual({ item: 'myTickets', title: 'My tickets', count: 2 })

    const vpn = rows(list).find((r) => r.ticket.ref === 'R-000412')!
    expect(vpn.priorityMark).toBe('p1')
    expect(vpn.time).toEqual({ type: 'spent', text: '1h 15m' })

    await mock.dispatch('list:mine', { type: 'select', ticketId: vpn.ticket.id })
    const detail = asTicket(await until(mock, 'detail:mine', (s) => s.type === 'ticket' && s.body.type === 'loaded'))
    expect(detail.primary).toMatchObject({ type: 'action', kind: 'openResolve', label: 'Resolve ticket', teal: false })
    expect(detail.strip.linkedCIs).toEqual([{ key: '55', name: 'vpn-gw-01', className: 'NetworkDevice' }])
    expect(detail.body.type === 'loaded' && detail.body.entries).toHaveLength(3)
    expect(detail.composer?.canSend).toBe(false)

    await mock.dispatch('detail:mine', { type: 'setDraft', text: 'Gateway timeout raised to 8 hours.' })
    expect(asTicket((await mock.getSnapshot('detail:mine'))!).composer?.canSend).toBe(true)
    const sent = await mock.dispatch('detail:mine', { type: 'send' })
    expect(sent).toBe(true)

    const after = asTicket((await mock.getSnapshot('detail:mine'))!)
    expect(after.composer?.draft).toBe('')
    const entries = after.body.type === 'loaded' ? after.body.entries : []
    expect(entries).toHaveLength(4)
    expect(entries[3]!.header.startsWith('Demo Agent · Today')).toBe(true)
    expect(entries[3]!.content.blocks[0]).toEqual({ type: 'text', runs: [{ type: 'text', text: 'Gateway timeout raised to 8 hours.', link: null }] })
  })

  it('keeps the draft and says why when iTop refuses the entry', async () => {
    const mock = makeMock()
    const list = await until(mock, 'list:mine', (s) => s.content.type === 'list')
    await mock.dispatch('list:mine', { type: 'select', ticketId: rows(list)[0]!.ticket.id })
    await until(mock, 'detail:mine', (s) => s.type === 'ticket' && s.body.type === 'loaded')
    await mock.dispatch('detail:mine', { type: 'setDraft', text: 'this will fail' })
    expect(await mock.dispatch('detail:mine', { type: 'send' })).toBe(false)
    const d = asTicket((await mock.getSnapshot('detail:mine'))!)
    expect(d.composer?.draft).toBe('this will fail')
    expect(d.composer?.status).toEqual({
      text: "iTop didn't save the entry. iTop returned error 100: Error: demo failure on request. Your draft is kept.",
      isError: true,
    })
  })

  it('validates images in Rust-style intake (type and count)', async () => {
    const mock = makeMock()
    const list = await until(mock, 'list:mine', (s) => s.content.type === 'list')
    await mock.dispatch('list:mine', { type: 'select', ticketId: rows(list)[0]!.ticket.id })
    await until(mock, 'detail:mine', (s) => s.type === 'ticket' && s.body.type === 'loaded')
    const png = await fileToIntake(new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0])], 'shot.png'), 40 * 1_048_576)
    const text = await fileToIntake(new File(['hello'], 'notes.txt'), 40 * 1_048_576)
    await mock.dispatch('detail:mine', { type: 'addImages', items: [png, text, png, png, png, png, png] })
    const d = asTicket((await mock.getSnapshot('detail:mine'))!)
    expect(d.composer?.images).toHaveLength(5)
    expect(d.composer?.canAddImages).toBe(false)
    expect(d.composer?.status?.text).toBe(
      "notes.txt isn't an image iTop Wrap can add. Use PNG, JPEG or GIF. An entry can have up to 5 images, so 1 wasn't added.",
    )
  })

  it('starts, ticks and stops the timer, saving the time', async () => {
    let now = new Date('2026-11-16T10:00:00')
    const mock = makeMock({ now: () => now })
    const list = await until(mock, 'list:mine', (s) => s.content.type === 'list')
    const vpn = rows(list).find((r) => r.ticket.ref === 'R-000412')!.ticket

    await mock.dispatch('list:mine', { type: 'select', ticketId: vpn.id })
    const before = asTicket(await until(mock, 'detail:mine', (s) => s.type === 'ticket'))
    expect(before.timerButton).toEqual({ type: 'start', enabled: true, help: 'Start timer (⌘T)' })

    await mock.dispatch('timer', { type: 'start', ticketId: vpn.id })
    now = new Date(now.getTime() + 42 * 60_000 + 5_000)
    const timer = await mock.getSnapshot('timer')
    expect(timer?.running).toMatchObject({ ref: 'R-000412', counter: '00:42:05', clock: '0:42' })
    expect(asTicket((await mock.getSnapshot('detail:mine'))!).timerButton).toEqual({ type: 'running' })
    expect((await mock.getSnapshot('menubar'))?.trayTitle).toBe('00:42:05  2')
    const running = await mock.getSnapshot('list:mine')
    expect(rows(running!).find((r) => r.ticket.id === vpn.id)?.time).toEqual({ type: 'running' })

    await mock.dispatch('timer', { type: 'stop' })
    const stopped = await mock.getSnapshot('timer')
    expect(stopped?.running).toBeNull()
    expect(stopped?.notice).toEqual({ text: 'Saved 0:42 to R-000412.', isError: false, canRetry: false, dismissLabel: 'Dismiss' })
    const strip = asTicket((await mock.getSnapshot('detail:mine'))!).strip
    expect(strip.spent).toBe('1h 57m spent')
  })

  it('keeps a failed time save for Retry when saving fails', async () => {
    let now = new Date('2026-11-16T10:00:00')
    const mock = makeMock({ now: () => now, timeFail: true })
    const list = await until(mock, 'list:mine', (s) => s.content.type === 'list')
    const vpn = rows(list).find((r) => r.ticket.ref === 'R-000412')!.ticket
    await mock.dispatch('timer', { type: 'start', ticketId: vpn.id })
    now = new Date(now.getTime() + 5 * 60_000)
    await mock.dispatch('timer', { type: 'stop' })
    const t = await mock.getSnapshot('timer')
    expect(t?.notice).toMatchObject({ isError: true, canRetry: true, dismissLabel: 'Discard this time' })
    expect(t?.notice?.text.startsWith("Couldn't save 0:05 to R-000412.")).toBe(true)
  })

  it('pages, filters by org and looks up a full ref', async () => {
    const mock = makeMock({ many: true })
    await mock.dispatch('shell', { type: 'select', item: 'teamTickets' })
    const team = await until(mock, 'list:team', (s) => s.content.type === 'list')
    expect(team.pager).toMatchObject({ type: 'pages', page: 1, label: '1–20 of 69', canPrevious: false, canNext: true })
    await mock.dispatch('list:team', { type: 'setPage', page: 4 })
    expect((await mock.getSnapshot('list:team'))?.pager).toMatchObject({ page: 4, label: '61–69 of 69', canNext: false })

    const mine = await until(mock, 'list:mine', (s) => s.content.type === 'list')
    expect(mine.orgFilter.visible).toBe(true)
    await mock.dispatch('list:mine', { type: 'toggleOrg', name: 'Org C' })
    expect(rows((await mock.getSnapshot('list:mine'))!).map((r) => r.ticket.ref)).toEqual(['R-000398'])

    await mock.dispatch('list:mine', { type: 'clearOrgFilter' })
    await mock.dispatch('list:mine', { type: 'setQuery', query: 'r-000512' })
    const found = await until(mock, 'list:mine', (s) => s.content.type === 'list' && s.content.body.extraSection !== null)
    expect(found.content.type === 'list' && found.content.body.extraSection?.title).toBe('Search result: not in your queue')
  })

  it('resolves through the sheet and keeps the ticket under "Resolved just now"', async () => {
    const mock = makeMock()
    const list = await until(mock, 'list:mine', (s) => s.content.type === 'list')
    const vpn = rows(list).find((r) => r.ticket.ref === 'R-000412')!.ticket
    await mock.dispatch('list:mine', { type: 'select', ticketId: vpn.id })
    await mock.dispatch('resolve:mine', { type: 'open', ticketId: vpn.id })
    const sheet = await mock.getSnapshot('resolve:mine')
    expect(sheet).toMatchObject({ type: 'open', title: 'Resolve R-000412' })
    expect(sheet?.type === 'open' && [sheet.service.selectedId, sheet.subcategory.selectedId, sheet.code.selectedId]).toEqual(['3', '12', 'assistance'])

    expect(await mock.dispatch('resolve:mine', { type: 'confirm' })).toBeNull()
    const tried = await mock.getSnapshot('resolve:mine')
    expect(tried?.type === 'open' && tried.solutionError).toBe('Describe what fixed it. The caller sees this.')

    await mock.dispatch('resolve:mine', { type: 'setSolution', solution: 'Raised the gateway timeout.' })
    expect(await mock.dispatch('resolve:mine', { type: 'confirm' })).toBeNull()
    expect((await mock.getSnapshot('resolve:mine'))?.type).toBe('closed')
    const after = await mock.getSnapshot('list:mine')
    const body = after?.content.type === 'list' ? after.content.body : null
    expect(body?.notice).toEqual({ text: 'R-000412 is resolved.', isError: false })
    expect(body?.extraSection?.title).toBe('Resolved just now')
    expect(after?.selection).toBe(vpn.id)
  })

  it('creates a ticket and opens it in My tickets, assigned to you', async () => {
    const mock = makeMock()
    await until(mock, 'list:mine', (s) => s.content.type === 'list')
    await mock.dispatch('shell', { type: 'newTicket' })
    const created = await mock.dispatch('newTicket:sheet', { type: 'create' })
    expect(created).toBeNull()
    expect(await mock.getSnapshot('newTicket:sheet')).toMatchObject({ type: 'open', missingText: 'Fill in the organization first.' })
    await mock.dispatch('newTicket:sheet', { type: 'setOrganization', id: '1' })
    await mock.dispatch('newTicket:sheet', { type: 'setCaller', id: '21' })
    await mock.dispatch('newTicket:sheet', { type: 'setService', id: '3' })
    await mock.dispatch('newTicket:sheet', { type: 'setSubcategory', id: '12' })
    await mock.dispatch('newTicket:sheet', { type: 'setTitle', title: 'Wi-Fi drops in room 4' })
    await mock.dispatch('newTicket:sheet', { type: 'setDescription', description: 'Since this morning.' })
    expect(await mock.dispatch('newTicket:sheet', { type: 'create' })).toBeNull()
    expect((await mock.getSnapshot('newTicket:sheet'))?.type).toBe('closed')
    const mine = await until(
      mock,
      'list:mine',
      (s) => s.content.type === 'list' && s.content.body.notice?.text === 'R-000601 is created and assigned to you.',
    )
    expect(mine.selection).toBe('UserRequest::601')
  })
})

describe('useModel', () => {
  it('follows snapshots and sends intents', async () => {
    const mock = makeMock()
    setStoreForTests(new ModelStore(mock))
    const { result } = renderHook(() => useModel('shell'))
    await waitFor(() => expect(result.current[0]?.selection).toBe('myTickets'))
    await act(async () => {
      await result.current[1]({ type: 'select', item: 'people' })
    })
    await waitFor(() => expect(result.current[0]?.selection).toBe('people'))
    expect(result.current[0]?.contentTitle).toBe('People')
  })
})
