/**
 * The Swift app's ITOPWRAP_DEMO data (Sources/ITopWrap/App.swift, DebugLaunch), ported for the
 * mock backend. UI fixture only: the real data comes from iTop through Rust.
 */
import type { RichBlock, TicketClass } from '../contract'

export interface MockTicket {
  key: string
  ticketClass: TicketClass
  ref: string
  title: string
  status: string
  priority: number
  callerName: string
  orgName: string
  orgId: string
  teamId: string
  teamName: string
  agentId: string
  agentName: string
  startDate: Date | null
  lastUpdate: Date | null
}

export interface MockEntry {
  id: number
  date: Date | null
  author: string
  text: string
  /** Extra blocks (images) after the text. */
  images: string[]
}

export interface MockDetail {
  descriptionText: string
  /** Hand-made rich blocks standing in for the parsed HTML; null = plain text only. */
  descriptionBlocks: RichBlock[] | null
  publicLog: MockEntry[]
  privateLog: MockEntry[]
  linkedCIs: { key: string; name: string; className: string }[]
  serviceId: string
  serviceName: string
  subcategoryId: string
  subcategoryName: string
}

export interface Ref {
  id: string
  name: string
}

export interface MockCI {
  key: string
  className: string
  name: string
  orgName: string
}

export interface MockCIDetail {
  fields: { label: string; value: string }[]
  description: string
  dependsOn: MockCI[]
  usedBy: MockCI[]
  openTickets: MockTicket[]
}

export interface MockOrg {
  id: string
  name: string
  code: string
}

export interface MockPerson {
  id: string
  displayName: string
  orgId: string
  orgName: string
  email: string
  phone: string
  function: string
  teams: Ref[]
}

export const SERVER = 'https://itop.example.com'
export const HOST = 'itop.example.com'

export const ME = {
  userId: '1',
  contactId: '2',
  login: 'agent.demo',
  displayName: 'Demo Agent',
  initials: 'DA',
  teams: [
    { id: '10', name: 'Network' },
    { id: '11', name: 'Service desk' },
  ] as Ref[],
}

const ORG_IDS: Record<string, string> = { 'Org A': '1', 'Org B': '2', 'Org C': '3' }

interface TicketSpec {
  agent: string
  agentName?: string
  team?: string
  teamName?: string
  prio?: number
  hours: number
  caller: string
  org?: string
}

function ticket(now: Date, ref: string, title: string, status: string, spec: TicketSpec): MockTicket {
  const org = spec.org ?? 'Org A'
  return {
    key: ref.slice(-3),
    ticketClass: ref.startsWith('I') ? 'Incident' : 'UserRequest',
    ref,
    title,
    status,
    priority: spec.prio ?? 3,
    callerName: spec.caller,
    orgName: org,
    orgId: ORG_IDS[org] ?? '0',
    teamId: spec.team ?? '10',
    teamName: spec.teamName ?? 'Network',
    agentId: spec.agent,
    agentName: spec.agentName ?? '',
    startDate: new Date(now.getTime() - spec.hours * 3_600_000),
    lastUpdate: null,
  }
}

/** Every demo ticket: mine, others in my teams, the not-in-queue one, resolved/closed, and the bulk ones. */
export function demoTickets(now: Date, many: boolean) {
  const sd = { team: '11', teamName: 'Service desk' }
  const mine = [
    ticket(now, 'R-000367', 'Badge reader offline at the entrance', 'escalated_ttr', { agent: '2', ...sd, prio: 2, hours: 96, caller: 'Caller 6' }),
    ticket(now, 'I-000088', 'Mail delivery delayed', 'escalated_tto', { agent: '0', ...sd, prio: 2, hours: 5, caller: 'Caller 2', org: 'Org B' }),
    ticket(now, 'R-000412', 'VPN drops every hour', 'assigned', { agent: '2', prio: 1, hours: 2, caller: 'Caller 1' }),
    ticket(now, 'R-000405', 'Shared drive access for a new project', 'assigned', { agent: '2', ...sd, hours: 26, caller: 'Caller 3' }),
    ticket(now, 'R-000398', 'Laptop setup for a new starter', 'pending', { agent: '2', ...sd, hours: 50, caller: 'Caller 4', org: 'Org C' }),
    ticket(now, 'R-000371', 'Printer queue stuck on floor 3', 'pending', { agent: '2', ...sd, prio: 4, hours: 74, caller: 'Caller 5' }),
  ]
  const others = [
    ticket(now, 'R-000421', 'Email signature update for the sales team', 'assigned', { agent: '3', agentName: 'Agent 3', ...sd, prio: 4, hours: 24, caller: 'Caller 8' }),
    ticket(now, 'R-000433', 'Projector in the boardroom shows no signal', 'escalated_ttr', { agent: '4', agentName: 'Agent 4', prio: 2, hours: 72, caller: 'Caller 10', org: 'Org C' }),
    ticket(now, 'R-000436', 'New phone for the reception desk', 'new', { agent: '0', ...sd, prio: 4, hours: 0.7, caller: 'Caller 11', org: 'Org B' }),
  ]
  const found = ticket(now, 'R-000512', 'Teams calls drop on the Finance floor', 'assigned', {
    agent: '5', agentName: 'Agent 2', team: '12', teamName: 'Collaboration', hours: 6, caller: 'Caller 7', org: 'Org B',
  })
  const closed = [
    ticket(now, 'R-000355', 'Monitor arm for desk 14', 'resolved', { agent: '2', ...sd, prio: 4, hours: 200, caller: 'Caller 12' }),
    ticket(now, 'R-000349', 'Guest Wi-Fi password for the event', 'closed', { agent: '2', prio: 3, hours: 420, caller: 'Caller 13', org: 'Org B' }),
  ]
  const bulk = many
    ? Array.from({ length: 60 }, (_, index) => {
        const i = index + 1
        return ticket(now, `R-${String(500 + i).padStart(6, '0')}`, `Bulk demo ticket ${i}`, ['assigned', 'pending', 'new'][i % 3]!, {
          agent: i % 4 === 0 ? '0' : '7', agentName: 'Agent 7', prio: 1 + (i % 4), hours: i * 5, caller: `Caller ${i}`,
        })
      })
    : []
  return { mine, others, found, closed, bulk }
}

/** A wide grey "screenshot" standing in for the pasted iTop image 900 (2400 × 900, like the demo PNG). */
export const DEMO_IMAGE =
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="2400" height="900" viewBox="0 0 2400 900">' +
      '<rect width="2400" height="900" fill="#ebebeb"/><rect x="2300" width="100" height="900" fill="#999"/>' +
      '<rect x="24" y="752" width="200" height="28" fill="#c4291c"/>' +
      '<text x="24" y="820" font-family="-apple-system, sans-serif" font-size="18">VPN disconnected: session timeout (60 min)</text></svg>',
  )

function entry(now: Date, id: number, author: string, hoursAgo: number, text: string): MockEntry {
  return { id, date: new Date(now.getTime() - hoursAgo * 3_600_000), author, text, images: [] }
}

export function demoDetails(now: Date): Record<string, MockDetail> {
  return {
    'R-000412': {
      descriptionText:
        'Since Monday the VPN disconnects at the top of every hour, sometimes in the middle of a call. Reconnecting works straight away.\nHappens both in the office and at home. Laptop: [model], VPN client: [version].',
      descriptionBlocks: [
        {
          type: 'text',
          runs: [
            {
              type: 'text',
              text: 'Since Monday the VPN disconnects at the top of every hour, sometimes in the middle of a call. Reconnecting works straight away.\nHappens both in the office and at home. Laptop: [model], VPN client: [version].\n\nThe error the client shows (see the ',
              link: null,
            },
            { type: 'text', text: 'VPN timeout article', link: 'https://kb.example.com/vpn-timeouts' },
            { type: 'text', text: '):', link: null },
          ],
        },
        { type: 'image', src: DEMO_IMAGE, ticketWebUrl: `${SERVER}/pages/UI.php?operation=details&class=UserRequest&id=412` },
        {
          type: 'text',
          runs: [
            { type: 'text', text: 'A screenshot the caller linked from elsewhere:\n\n', link: null },
            { type: 'outsideImage', host: 'images.example.net', url: 'https://images.example.net/screenshot.png' },
            { type: 'text', text: '\n\nCaller 1\nSales\n', link: null },
            { type: 'icon' },
            { type: 'text', text: ' : +00 0000 0001\n', link: null },
            { type: 'icon' },
            { type: 'text', text: ' : ', link: null },
            { type: 'text', text: 'caller.one@example.com', link: 'mailto:caller.one@example.com' },
          ],
        },
      ],
      publicLog: [
        entry(now, 1, 'Caller 1', 2.8, 'The VPN disconnects at the top of every hour and I have to sign in again.'),
        entry(now, 2, 'Demo Agent', 1.3, 'Thanks, checking the gateway session timeout now.'),
        entry(now, 3, 'Caller 1', 0.9, 'It happened again at 11:00.'),
      ],
      privateLog: [entry(now, 4, 'Demo Agent', 1.25, 'Gateway timeout looks like 60 min. Confirm with Team Network before changing.')],
      linkedCIs: [{ key: '55', name: 'vpn-gw-01', className: 'NetworkDevice' }],
      serviceId: '3',
      serviceName: 'Network access',
      subcategoryId: '12',
      subcategoryName: 'VPN',
    },
  }
}

export function emptyDetail(): MockDetail {
  return {
    descriptionText: 'Demo ticket without a description.',
    descriptionBlocks: null,
    publicLog: [],
    privateLog: [],
    linkedCIs: [],
    serviceId: '',
    serviceName: '',
    subcategoryId: '',
    subcategoryName: '',
  }
}

export const ATTACHMENTS: Record<string, Ref[]> = {
  'R-000412': [
    { id: '1', name: 'vpn-client.log' },
    { id: '2', name: 'gateway-config.txt' },
  ],
}

export const SERVICES: Ref[] = [
  { id: '3', name: 'Network access' },
  { id: '4', name: 'Workplace' },
  { id: '5', name: 'Collaboration' },
]

export const SUBCATEGORIES: Record<string, Ref[]> = {
  '3': [
    { id: '12', name: 'VPN' },
    { id: '13', name: 'Wi-Fi' },
  ],
  '4': [
    { id: '20', name: 'Laptop' },
    { id: '21', name: 'Printer' },
  ],
  '5': [{ id: '30', name: 'Teams calls' }],
}

export const TEAMS: Ref[] = [
  { id: '10', name: 'Network' },
  { id: '11', name: 'Service desk' },
  { id: '12', name: 'Collaboration' },
]

export const AGENTS: Record<string, Ref[]> = {
  '10': [
    { id: '2', name: 'Demo Agent' },
    { id: '4', name: 'Agent 4' },
  ],
  '11': [
    { id: '2', name: 'Demo Agent' },
    { id: '3', name: 'Agent 3' },
  ],
  '12': [{ id: '5', name: 'Agent 2' }],
}

export const RESOLUTION_CODES = ['assistance', 'bug fixed', 'hardware repair', 'other', 'software patch', 'system update', 'training']

export const IMPACTS: Ref[] = [
  { id: '1', name: 'A department' },
  { id: '2', name: 'A service' },
  { id: '3', name: 'A person' },
]

export const URGENCIES: Ref[] = [
  { id: '1', name: 'Critical' },
  { id: '2', name: 'High' },
  { id: '3', name: 'Medium' },
  { id: '4', name: 'Low' },
]

export const NEW_TICKET_ORGS: Ref[] = [
  { id: '1', name: 'Org A' },
  { id: '2', name: 'Org B' },
  { id: '3', name: 'Org C' },
]

export const CALLERS: Record<string, Ref[]> = {
  '1': [
    { id: '21', name: 'Caller One' },
    { id: '23', name: 'Caller Three' },
  ],
  '2': [
    { id: '22', name: 'Caller Two' },
    { id: '27', name: 'Caller Seven' },
  ],
  '3': [{ id: '24', name: 'Caller Four' }],
}

/** Seconds already saved per ticket id (demoTimer totals). */
export const BASE_TOTALS: Record<string, number> = {
  'UserRequest::412': 4500,
  'UserRequest::405': 1800,
  'UserRequest::367': 9000,
}

/** The ticket with a running web stopwatch, started 25 minutes before launch. */
export const WEB_STOPWATCH_REF = 'R-000405'

/** "Agent 3" took it a moment ago. */
export const RACE_REF = 'R-000436'

export function demoCMDB(now: Date) {
  const gw1: MockCI = { key: '55', className: 'NetworkDevice', name: 'vpn-gw-01', orgName: 'Org A' }
  const gw2: MockCI = { key: '56', className: 'NetworkDevice', name: 'vpn-gw-02', orgName: 'Org A' }
  const app: MockCI = { key: '70', className: 'ApplicationSolution', name: 'Remote access VPN', orgName: 'Org A' }
  const device = [
    { label: 'Status', value: 'Production' },
    { label: 'Organization', value: 'Org A' },
    { label: 'Location', value: '[Location]' },
    { label: 'Brand', value: '[Brand]' },
    { label: 'Model', value: '[Model]' },
    { label: 'Serial number', value: '[Serial]' },
    { label: 'Management IP', value: '[IP address]' },
    { label: 'Contacts', value: 'Team Network' },
  ]
  const t = (ref: string, title: string, status: string, agent: string, name: string): MockTicket => ({
    key: ref.slice(-3), ticketClass: 'UserRequest', ref, title, status, priority: 3, callerName: 'Caller', orgName: 'Org A', orgId: '1',
    teamId: '10', teamName: 'Network', agentId: agent, agentName: name, startDate: new Date(now.getTime() - 7_200_000), lastUpdate: null,
  })
  const vpn = t('R-000412', 'VPN drops every hour', 'assigned', '2', 'Demo Agent')
  const details: Record<string, MockCIDetail> = {
    'NetworkDevice::55': { fields: device, description: '', dependsOn: [], usedBy: [app], openTickets: [vpn] },
    'NetworkDevice::56': { fields: [{ label: 'Status', value: 'Standby' }, ...device.slice(1)], description: '', dependsOn: [], usedBy: [app], openTickets: [] },
    'ApplicationSolution::70': {
      fields: [
        { label: 'Status', value: 'Active' },
        { label: 'Organization', value: 'Org A' },
        { label: 'Business criticality', value: 'High' },
        { label: 'Contacts', value: 'Team Network' },
      ],
      description: 'Remote access for staff working away from the office.',
      dependsOn: [gw1, gw2],
      usedBy: [],
      openTickets: [vpn, t('R-000498', 'VPN profile for a contractor', 'escalated_ttr', '5', 'Agent 2')],
    },
  }
  return { items: [app, gw1, gw2], details }
}

export const ORGS: MockOrg[] = [
  { id: '1', name: 'Org A', code: 'A' },
  { id: '2', name: 'Org B', code: 'B' },
  { id: '3', name: 'Org C', code: 'C' },
]

export const ORG_COUNTS: Record<string, { people: number; configItems: number }> = {
  '1': { people: 6, configItems: 3 },
  '2': { people: 3, configItems: 0 },
  '3': { people: 3, configItems: 0 },
}

export const PEOPLE: MockPerson[] = [
  { id: '2', displayName: 'Demo Agent', orgId: '1', orgName: 'Org A', email: 'agent.demo@example.com', phone: '', function: 'IT support', teams: ME.teams },
  { id: '4', displayName: 'Agent Four', orgId: '1', orgName: 'Org A', email: 'agent.four@example.com', phone: '', function: 'Facilities', teams: [{ id: '10', name: 'Network' }] },
  { id: '21', displayName: 'Caller One', orgId: '1', orgName: 'Org A', email: 'caller.one@example.com', phone: '+00 0000 0001', function: 'Sales', teams: [] },
  { id: '22', displayName: 'Caller Two', orgId: '2', orgName: 'Org B', email: 'caller.two@example.com', phone: '', function: '', teams: [] },
]

/** Which caller name on the demo tickets belongs to which person. */
export const CALLER_OF: Record<string, string> = { '21': 'Caller 1', '22': 'Caller 2' }
