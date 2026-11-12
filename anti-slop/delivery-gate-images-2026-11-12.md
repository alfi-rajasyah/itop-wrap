# Delivery Gate: images in log entries (2026-11-12)

antislop mode: during (session override, from the task brief). Scope: the log composer (paste, drop, paperclip, thumbnails, send). Dials unchanged from DESIGN.md: ENERGY 1 / RHYTHM 1 / MOTION 1.

Design Read: native macOS work tool for an IT agent adding a screenshot to a ticket log, calm platform-native style, dial 1/1/1.

## Click-through (R-35), demo build `ITOPWRAP_DEMO=1`, this branch's own bundle

| Step | Result |
|---|---|
| ⌘V with a PNG on the clipboard, text typed | Thumbnail appears under the text with a remove button; text untouched |
| ⌘↩ with text and one image | "Uploading the image to iTop" and a spinner, then the entry shows with the image inside it; draft cleared |
| Text with "refuse", one image, ⌘↩ | "iTop didn't save the entry. iTop returned error 100: Error: contents: demo refusal, the image was not stored. Your draft is kept."; draft and image stay |
| Drag test-shot.png from Finder onto the entry box | Second thumbnail added, error cleared, no file path inserted in the text |
| Remove button on a thumbnail | That image goes, the other stays |
| ⌘V seven times | Five thumbnails; "An entry can have up to 5 images, so 1 weren't added." (now reads "1 wasn't added") |
| ⌘↩ with five images | "Uploading 5 images to iTop"; remove buttons disabled; the send button keeps its full label (it was squeezed before the fix) |
| Click the posted image | Full-size viewer opens; Escape closes it |
| Paperclip click | File picker with only image files enabled; Escape closes it |
| Keyboard: Tab from the entry box | Focus moves to the paperclip with a visible ring (Tab used to insert a tab character); Space opens the picker; Escape closes it |
| Light and dark | Both checked on screen: thumbnails, remove buttons, error and status lines readable |
| VoiceOver names (accessibility tree) | "Public entry" (text area), "Add images", "Remove Pasted image.png", thumbnail as an image named "filename, size" |

## Block 1: Hard Gate

- R-02 PASS: no em dash in any new UI string (checked TicketDetailView, ComposerTextView, TicketDetailModel).
- R-03 N/A: macOS window, not a mobile layout; the narrow window was checked and the status line wraps instead of squeezing the button.
- R-17, R-18, R-28, R-36, R-38 PASS: no numbers, testimonials, FAQ or claims added. The limits shown (4 MB, 5 images) are the real ones the code enforces.
- R-23 PASS: no visual assets created; the test image was the existing app icon converted in the scratchpad, not added to the repo.
- R-24 N/A: no navigation added.
- R-25 PASS: error text uses Theme.warning and notes Theme.secondary, both already checked for contrast in DESIGN.md; the remove button is white on 70 % black.
- R-26 PASS: every new control acts (paperclip opens the picker, remove removes, send sends).
- R-27 PASS: no images (nothing shown), adding (thumbnails), uploading (status and spinner), refused (iTop's message), over the limit (message).
- R-32 PASS: Tab reaches the paperclip, Space activates it, Escape closes the picker; system focus ring visible.
- R-33 PASS: written in source, no patch scripts.
- R-34 PASS: light and dark both checked on screen.
- R-35 PASS: click-through above.
- R-37 PASS: DESIGN.md direction applied; new section "Images in the log composer".
- R-39 PASS: native Buttons with accessibility names, thumbnail has an image trait and name, disabled states for paperclip and remove buttons.
- R-40 N/A: not a web page.

## Block 2: Purpose-Gate

- R-01, R-07, R-10, R-13 PASS: no gradients, backgrounds, blur or glow.
- R-04 PASS: `paperclip` (adding a file to a message) and `xmark.circle.fill` (remove), both standard meanings.
- R-12 PASS: no shadows added.
- Others N/A.

## Block 3: Liveliness

PASS: dials 1/1/1 declared and kept; the focal point stays the send button; teal not used for anything new; the identity motif is untouched.

## Block 4: Craftsmanship and Quality Locks

PASS: every decision has its reason in DESIGN.md "Images in the log composer"; no template layout, no buzzwords, no new colours.

Not checked: a real upload to the user's iTop (the user chose to skip it), and VoiceOver spoken output (checked through the accessibility tree, not by listening).
