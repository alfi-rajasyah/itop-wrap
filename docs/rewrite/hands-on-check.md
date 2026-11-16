# Hands-on check before merging

The rewrite was built and tested with the Mac's screen locked, so nothing below was seen by eye.
Everything else is covered by tests (644 Rust, 147 UI) and two live runs against a test iTop.

Open `build/iTop Wrap.app` (after `scripts/bundle-app.sh`). The first launch asks to use the Keychain
item the Swift app saved: choose **Always Allow**.

1. **Windows.** Sign-in window is small and fixed; after sign-in the shell opens, resizes, and
   remembers its frame. The red close button hides the window and the app keeps running; the Dock icon
   and ⌘O bring it back. Dragging the top bar moves the window; the traffic lights don't overlap content.
2. **Menu bar item.** Icon plus the needs-you count. Click opens the panel under it; clicking elsewhere
   closes it. Start a timer: the time slides out of the icon and ticks every second. ⌘N, ⌘O and ⌘R work
   inside the panel. The panel shows over a full-screen app.
3. **Menus.** File ▸ New Ticket… (⌘N), Open iTop Wrap (⌘O); View ▸ Hide Sidebar (⌃⌘S), ⌘1–⌘5; Edit ▸
   Copy and Paste work in text fields.
4. **New Ticket window** (from the panel): opens centred, height fits the form, its close button
   discards without keeping a draft.
5. **Images.** Paste a screenshot (⌘V), paste an image file copied in Finder, drop an image on the
   composer, pick one with the paperclip. A HEIC photo is converted. Dropping a link elsewhere in the
   window does nothing.
6. **Attachments.** Download one: the Save panel appears and the file is written.
7. **Copy.** Click a ticket ref: "Copied R-…" appears under the title and the ref is on the clipboard.
8. **Look.** Light and dark, sidebar drag (180–260), account circle at the bottom left.
9. **Time tracking.** The test iTop has no Time Tracking extension, so saving a time entry and the web
   stopwatch were only tested against recorded responses. Start and stop a timer on a real Assigned
   ticket of yours and check the entry in iTop.
10. **Pending tickets.** A stock iTop refuses Resolve and Reassign on a pending ticket (the Swift app
    offers them too). Check both on your own iTop, whose lifecycle is customised.

The Swift sources (`Package.swift`, `Sources/`, `Tests/`, `Resources/`) are still in the repo so the
two apps can be compared. Remove them once this list passes.
