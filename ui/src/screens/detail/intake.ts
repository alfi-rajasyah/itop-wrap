import { fileToIntake, type ImageIntakeItem } from '@/backend'

/* Which pasted or dropped files go to `detail.addImages` (LOG-14, LOG-15). Rust judges the
 * bytes, converts, and words every message; this only mirrors which clipboard flavour the
 * Swift composer took, as the contract leaves the `paste` event to React (§6 Clipboard). */

const IMAGE_NAME = /\.(png|jpe?g|gif|heic|heif|tiff?|bmp|webp|avif)$/i

function isImageFile(file: File): boolean {
  return file.type.startsWith('image/') || IMAGE_NAME.test(file.name)
}

/** Files copied in Finder come with their file URLs; a screenshot copied to the clipboard doesn't. */
function hasFileUrls(data: DataTransfer): boolean {
  return Array.from(data.types).includes('text/uri-list')
}

export type IntakePick = { files: File[]; raw: boolean }

/**
 * Paste: image files win; files without images paste normally; text pastes as text; otherwise
 * raw image data (PNG first, then TIFF) goes in unnamed, so Rust names it "Pasted image…".
 * Null means "let the editor paste normally".
 */
export function pickPasted(data: DataTransfer): IntakePick | null {
  const files = Array.from(data.files)
  if (files.length === 0) return null
  const images = files.filter(isImageFile)
  if (hasFileUrls(data)) return images.length > 0 ? { files: images, raw: false } : null
  if (Array.from(data.types).includes('text/plain')) return null
  const raw = images.find((f) => f.type === 'image/png') ?? images.find((f) => f.type === 'image/tiff') ?? images[0]
  return raw ? { files: [raw], raw: true } : null
}

/** Drop: the same, but image data is taken even when text comes along. */
export function pickDropped(files: File[]): IntakePick | null {
  const images = files.filter(isImageFile)
  return images.length > 0 ? { files: images, raw: false } : null
}

export async function toIntake(pick: IntakePick, maxBytes: number): Promise<ImageIntakeItem[]> {
  const items = await Promise.all(pick.files.map((f) => fileToIntake(f, maxBytes)))
  return pick.raw ? items.map((item) => ({ ...item, filename: '' })) : items
}
