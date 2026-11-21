/**
 * Type-level conformance: the types ts-rs generates from the Rust models (ui/src/bindings/app/,
 * `npm run bindings`) must be mutually assignable with the hand-written contract (contract.ts),
 * snapshot and intent of every model. Compiled by `tsc -b`; nothing here runs.
 */
import type * as C from './contract'

import type { SessionSnapshot } from '../bindings/app/SessionSnapshot'
import type { SessionIntent } from '../bindings/app/SessionIntent'
import type { SignInResult } from '../bindings/app/SignInResult'
import type { ShellSnapshot } from '../bindings/app/ShellSnapshot'
import type { ShellIntent } from '../bindings/app/ShellIntent'
import type { OptionsSnapshot } from '../bindings/app/OptionsSnapshot'
import type { OptionsIntent } from '../bindings/app/OptionsIntent'
import type { ListSnapshot } from '../bindings/app/ListSnapshot'
import type { ListIntent } from '../bindings/app/ListIntent'
import type { DetailSnapshot } from '../bindings/app/DetailSnapshot'
import type { DetailIntent } from '../bindings/app/DetailIntent'
import type { AssignSheetSnapshot } from '../bindings/app/AssignSheetSnapshot'
import type { AssignSheetIntent } from '../bindings/app/AssignSheetIntent'
import type { PendingSheetSnapshot } from '../bindings/app/PendingSheetSnapshot'
import type { PendingSheetIntent } from '../bindings/app/PendingSheetIntent'
import type { ResolveSheetSnapshot } from '../bindings/app/ResolveSheetSnapshot'
import type { ResolveSheetIntent } from '../bindings/app/ResolveSheetIntent'
import type { NewTicketSnapshot } from '../bindings/app/NewTicketSnapshot'
import type { NewTicketIntent } from '../bindings/app/NewTicketIntent'
import type { TimerSnapshot } from '../bindings/app/TimerSnapshot'
import type { TimerIntent } from '../bindings/app/TimerIntent'
import type { CMDBSnapshot } from '../bindings/app/CMDBSnapshot'
import type { CMDBIntent } from '../bindings/app/CMDBIntent'
import type { OrgsSnapshot } from '../bindings/app/OrgsSnapshot'
import type { OrgsIntent } from '../bindings/app/OrgsIntent'
import type { PeopleSnapshot } from '../bindings/app/PeopleSnapshot'
import type { PeopleIntent } from '../bindings/app/PeopleIntent'
import type { MenuBarSnapshot } from '../bindings/app/MenuBarSnapshot'
import type { MenuBarIntent } from '../bindings/app/MenuBarIntent'
import type { UpdateSnapshot } from '../bindings/app/UpdateSnapshot'
import type { UpdateIntent } from '../bindings/app/UpdateIntent'
import type { SettingsSnapshot } from '../bindings/app/SettingsSnapshot'
import type { SettingsIntent } from '../bindings/app/SettingsIntent'
import type { AppError } from '../bindings/AppError'

/** True when each type is assignable to the other. */
type Mutual<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false
/** Fails to compile unless `T` is `true`. */
type Assert<T extends true> = T

export type Conformance = [
  Assert<Mutual<SessionSnapshot, C.SessionSnapshot>>,
  Assert<Mutual<SessionIntent, C.SessionIntent>>,
  Assert<Mutual<SignInResult, C.SignInResult>>,
  Assert<Mutual<ShellSnapshot, C.ShellSnapshot>>,
  Assert<Mutual<ShellIntent, C.ShellIntent>>,
  Assert<Mutual<OptionsSnapshot, C.OptionsSnapshot>>,
  Assert<Mutual<OptionsIntent, C.OptionsIntent>>,
  Assert<Mutual<ListSnapshot, C.ListSnapshot>>,
  Assert<Mutual<ListIntent, C.ListIntent>>,
  Assert<Mutual<DetailSnapshot, C.DetailSnapshot>>,
  Assert<Mutual<DetailIntent, C.DetailIntent>>,
  Assert<Mutual<AssignSheetSnapshot, C.AssignSheetSnapshot>>,
  Assert<Mutual<AssignSheetIntent, C.AssignSheetIntent>>,
  Assert<Mutual<PendingSheetSnapshot, C.PendingSheetSnapshot>>,
  Assert<Mutual<PendingSheetIntent, C.PendingSheetIntent>>,
  Assert<Mutual<ResolveSheetSnapshot, C.ResolveSheetSnapshot>>,
  Assert<Mutual<ResolveSheetIntent, C.ResolveSheetIntent>>,
  Assert<Mutual<NewTicketSnapshot, C.NewTicketSnapshot>>,
  Assert<Mutual<NewTicketIntent, C.NewTicketIntent>>,
  Assert<Mutual<TimerSnapshot, C.TimerSnapshot>>,
  Assert<Mutual<TimerIntent, C.TimerIntent>>,
  Assert<Mutual<CMDBSnapshot, C.CMDBSnapshot>>,
  Assert<Mutual<CMDBIntent, C.CMDBIntent>>,
  Assert<Mutual<OrgsSnapshot, C.OrgsSnapshot>>,
  Assert<Mutual<OrgsIntent, C.OrgsIntent>>,
  Assert<Mutual<PeopleSnapshot, C.PeopleSnapshot>>,
  Assert<Mutual<PeopleIntent, C.PeopleIntent>>,
  Assert<Mutual<MenuBarSnapshot, C.MenuBarSnapshot>>,
  Assert<Mutual<MenuBarIntent, C.MenuBarIntent>>,
  Assert<Mutual<UpdateSnapshot, C.UpdateSnapshot>>,
  Assert<Mutual<UpdateIntent, C.UpdateIntent>>,
  Assert<Mutual<SettingsSnapshot, C.SettingsSnapshot>>,
  Assert<Mutual<SettingsIntent, C.SettingsIntent>>,
  Assert<Mutual<AppError, C.AppError>>,
]
