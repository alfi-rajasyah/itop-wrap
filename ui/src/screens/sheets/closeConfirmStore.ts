import { create } from 'zustand'
import type { ListScope } from '@/backend'

/**
 * Which list's detail asked "Close <ref>?" (DET-16). Purely visual state (contract §1 rule 3):
 * the detail's primary button sets it, TicketSheets shows the confirmation with the detail
 * snapshot's `closeConfirm` texts.
 */
type CloseConfirmState = {
  request: { scope: ListScope; ticketId: string } | null
  ask: (scope: ListScope, ticketId: string) => void
  dismiss: () => void
}

export const useCloseConfirm = create<CloseConfirmState>((set) => ({
  request: null,
  ask: (scope, ticketId) => set({ request: { scope, ticketId } }),
  dismiss: () => set({ request: null }),
}))
