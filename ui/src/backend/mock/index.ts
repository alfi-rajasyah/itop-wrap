/**
 * MOCK BACKEND entry point. A UI fixture for the browser and for tests, NOT the source of truth:
 * see backend.ts. URL switches (the Swift ITOPWRAP_DEMO_* equivalents):
 *
 *   ?state=empty|loading|error   forced list state (ITOPWRAP_DEMO_STATE)
 *   ?signedOut=1                 start on the sign-in form
 *   ?unreachable=1               start on "Can't reach iTop"
 *   ?timeFail=1                  saving time fails (ITOPWRAP_DEMO_TIMEFAIL)
 *   ?many=1                      60 bulk tickets in Team tickets (ITOPWRAP_DEMO_MANY)
 *   ?readOnly=1                  writes switched off, like a debug build
 *   ?delay=<ms>                  simulated latency (default 300)
 */
import { DEFAULT_MOCK_OPTIONS, MockBackend, type MockOptions } from './backend'

export { MockBackend, DEFAULT_MOCK_OPTIONS, type MockOptions } from './backend'

export function mockOptionsFromSearch(search: string): Partial<MockOptions> {
  const params = new URLSearchParams(search)
  const flag = (name: string) => params.get(name) === '1' || params.get(name) === 'true'
  const state = params.get('state')
  const delay = Number(params.get('delay'))
  return {
    listState: state === 'empty' || state === 'loading' || state === 'error' ? state : 'normal',
    signedOut: flag('signedOut'),
    unreachable: flag('unreachable'),
    timeFail: flag('timeFail'),
    many: flag('many'),
    readOnly: flag('readOnly'),
    delayMs: Number.isFinite(delay) && params.has('delay') ? delay : DEFAULT_MOCK_OPTIONS.delayMs,
  }
}

export function createMockBackend(options: Partial<MockOptions> = {}): MockBackend {
  return new MockBackend(options)
}
