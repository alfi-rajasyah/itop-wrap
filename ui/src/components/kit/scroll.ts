/** Scrolls `el` into view inside its nearest scrolling ancestor only. Unlike
 *  Element.scrollIntoView it never scrolls the window or outer panes. */
export function scrollIntoNearest(el: HTMLElement | null) {
  if (!el) return;
  let parent = el.parentElement;
  while (parent && parent !== document.body) {
    const oy = getComputedStyle(parent).overflowY;
    if ((oy === "auto" || oy === "scroll") && parent.scrollHeight > parent.clientHeight) break;
    parent = parent.parentElement;
  }
  if (!parent || parent === document.body) return;
  const p = parent.getBoundingClientRect();
  const r = el.getBoundingClientRect();
  // leave room for a sticky section header
  const pad = 30;
  if (r.top < p.top + pad) parent.scrollTop -= p.top + pad - r.top;
  else if (r.bottom > p.bottom) parent.scrollTop += r.bottom - p.bottom;
}
