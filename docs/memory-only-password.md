# Spec: Memory-only password and automatic sign-out

## Objective

iTop Wrap stops saving your password anywhere on disk. It lives only in memory while you're
signed in, and the app signs you out by itself after 12 hours without use. Agreed with you on
2026-11-17, after deciding against the login keychain and against the data protection keychain
(that needs a provisioning profile, which a free Apple ID only gets for 7 days at a time).

## What you'll see

1. **Sign in** works as today. The server and username are still remembered; the password isn't.
   The sign-in screen says: "Your password isn't saved. iTop Wrap signs you out after 12 hours
   without use, and when it quits."
2. **Launch** (including Open at Login and after a restart) always shows the sign-in screen with
   the server and username filled in and the password field focused.
3. **Use** means anything you do in the main window, the menu bar panel, the app's menus or its
   other windows: clicking, typing, opening a ticket, refreshing. Background work doesn't count:
   the 2-minute My tickets check, the team poll, a ticking timer, notifications.
4. **After 12 hours without use**, the app signs you out:
   - A running ticket timer is stopped and its time saved to iTop first, while the password is
     still known.
   - If that save fails (for example you're offline), the timer is left running instead, as a
     manual sign-out does today, so no time is lost. You stop it after signing in again.
   - The sign-in screen shows: "You were signed out after 12 hours without use. Sign in
     again." plus, when a timer was saved, "Saved 1:23 to R-000601." or, when it couldn't be,
     "Your timer on R-000601 is still running."
   - The countdown runs on the wall clock and restarts with every use, so time asleep counts
     (last use at 18:00, the Mac sleeps until 07:00 the next day, and you're signed out when
     it wakes).
   - A timer left running with no use for 12 hours is past iTop's 4-hour stopwatch limit
     anyway, so it saves 4:00, as it does today (SPEC.md, Long timers).
5. **Sign out** by hand works as today; its confirmation no longer mentions the Keychain:
   "Your username stays filled in."
6. **Upgrading:** on its first launch, the new version deletes the password the old version
   saved in the login keychain. macOS may ask once to allow that.

## Out of scope

- A setting for the time limit (fixed at 12 hours for now).
- Signing out when the Mac locks, or a fixed limit counted from sign-in.
- Any Keychain or Passwords app integration.

## How it's built

- `SessionService` keeps getting a `SecretStore`, but the app now gives it the in-memory store
  (`MemorySecretStore`). Sign-in puts the password there, sign-out removes it, and on launch it's
  empty, so `restore()` finds nothing and shows the sign-in form. No kit changes are needed.
- `AppRouter` records the last use (`Native::now()`, the wall clock): set at sign-in, and on
  every intent from the UI and every native event except launch. Background loops don't go
  through the router, so they can't reset it. A check runs every 30 seconds while you're signed
  in; at 12 hours since the last use it stops and saves a running timer, then signs out with
  the notice above.
- `keychain.rs` keeps only what's needed to delete the old item at launch.
- Copy changes: the sign-in line, the sign-out confirmation, README, SPEC.md, UX-FLOWS.md,
  DESIGN.md (Account tab).

## Testing

- Rust model tests (`cargo test --workspace`), with `FakeNative`'s clock moved forward and tokio's
  paused time advanced:
  - Launch with a server and username saved shows the sign-in form, password focused.
  - Signed in at T, no use: still signed in at T + 11:59, signed out with the notice at
    T + 12:00.
  - Used at T + 10:00: still signed in at T + 21:59, signed out at T + 22:00.
  - Background refreshes alone don't keep you signed in.
  - A running timer at the limit is saved to iTop before signing out, and the notice says so.
  - A timer whose save fails at the limit is still running after sign-out.
  - Signing out by hand, then in again, starts a fresh 12 hours.
- UI tests (`npm --prefix ui test`) for the new copy.
- By hand on the bundled app: the old Keychain item is gone after first launch (Keychain Access).

## Boundaries

- Always: never write the password to disk, logs or traces; run both test suites before each
  commit.
- Ask first: changing the 12-hour limit or what counts as use, or what happens to a running timer.
- Never: keep a copy of the password anywhere outside memory.
