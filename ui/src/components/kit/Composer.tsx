import {
  forwardRef,
  useRef,
  useState,
  type ClipboardEvent,
  type DragEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { cn } from "@/lib/utils";
import { Button } from "./Button";
import { PaperclipIcon, SendIcon, XIcon } from "./icons";
import { IconButton } from "./IconButton";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * COMPOSER — the log entry box (LOG-1…23), adapted from Prompt
 * Bar's Rounded variant: the 14px card with a hairline border that
 * darkens on focus, the attachment row above the controls, the 28px
 * icon button on the left and the ink send button on the right.
 * Dropped: the demo walkthrough, slash menu, model picker,
 * dictation and the glimm shader. Changed: a multi-line 64px editor
 * (Return is a new line, ⌘↩ sends), image thumbnails instead of file
 * pills, a labelled send button ("Add to public log"), and a status
 * line between paperclip and send that wraps instead of squeezing
 * the button.
 *
 * Paste and drop are passed to the caller (it decides what's an
 * image); the box darkens while files are dragged over it.
 * ───────────────────────────────────────────────────────── */

export type ComposerImage = {
  id: string;
  /** Object URL or data: URL for the thumbnail. */
  src: string;
  name: string;
  /** "1.2 MB". */
  sizeLabel?: string;
};

export type ComposerProps = {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  placeholder?: string;
  /** Accessible name of the editor ("Public entry"). */
  label: string;
  sendLabel: ReactNode;
  sendDisabled?: boolean;
  sendTooltip?: ReactNode;
  busy?: boolean;
  /** Writes off: the editor can't be edited. */
  readOnly?: boolean;
  images?: ComposerImage[];
  onRemoveImage?: (id: string) => void;
  /** Paperclip: open the image picker. */
  onAttach?: () => void;
  attachDisabled?: boolean;
  attachTooltip?: ReactNode;
  /** Status line text: an error, an upload message, or the writes-off notice. */
  status?: ReactNode;
  statusTone?: "info" | "error";
  onPaste?: (event: ClipboardEvent<HTMLTextAreaElement>) => void;
  /** Files dropped on the box; true when taken (otherwise the default drop happens). */
  onDropFiles?: (files: File[], event: DragEvent<HTMLDivElement>) => boolean;
  className?: string;
};

export const Composer = forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  {
    value,
    onChange,
    onSend,
    placeholder,
    label,
    sendLabel,
    sendDisabled = false,
    sendTooltip = "Add the entry (⌘↩)",
    busy = false,
    readOnly = false,
    images = [],
    onRemoveImage,
    onAttach,
    attachDisabled = false,
    attachTooltip,
    status,
    statusTone = "info",
    onPaste,
    onDropFiles,
    className,
  },
  ref,
) {
  const [dragging, setDragging] = useState(false);
  const depth = useRef(0);
  const canSend = !sendDisabled && !busy;

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && !e.nativeEvent.isComposing) {
      e.preventDefault();
      e.stopPropagation();
      if (canSend) onSend();
    }
  };

  // LOG-15: only a drag that carries images highlights the box and is taken; any other drag
  // gets the default drop. While dragging only the items' types are readable; a file whose type
  // WebKit doesn't report counts, and the caller rejects it on drop if it isn't an image.
  const hasFiles = (e: DragEvent) => {
    const items = Array.from(e.dataTransfer?.items ?? []).filter((i) => i.kind === "file");
    if (items.length > 0) return items.some((i) => i.type === "" || i.type.startsWith("image/"));
    return Array.from(e.dataTransfer?.types ?? []).includes("Files");
  };

  return (
    <div
      data-dropzone
      onDragEnter={(e) => {
        if (!hasFiles(e) || readOnly) return;
        depth.current += 1;
        setDragging(true);
      }}
      onDragOver={(e) => {
        if (!hasFiles(e) || readOnly) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = "copy";
      }}
      onDragLeave={(e) => {
        if (!hasFiles(e)) return;
        depth.current = Math.max(0, depth.current - 1);
        if (depth.current === 0) setDragging(false);
      }}
      onDrop={(e) => {
        depth.current = 0;
        setDragging(false);
        if (readOnly || !hasFiles(e)) return;
        if (onDropFiles?.(Array.from(e.dataTransfer.files), e)) e.preventDefault();
      }}
      className={cn(
        "relative flex flex-col gap-1.5 rounded-[14px] border bg-surface p-1.5 shadow-card transition-[border-color,background-color,border-width] duration-150",
        dragging ? "border-2 border-ink-2 bg-inset p-[5px]" : "border-line focus-within:border-ink-3",
        className,
      )}
    >
      <textarea
        ref={ref}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
        readOnly={readOnly}
        placeholder={placeholder}
        aria-label={label}
        aria-keyshortcuts="Meta+Enter"
        spellCheck
        autoCorrect="off"
        className="h-16 min-h-16 w-full resize-none bg-transparent px-1.5 py-1 text-[13px] leading-[18px] text-ink outline-none [overflow-wrap:anywhere] placeholder:text-ink-2/80 read-only:cursor-default"
      />

      {images.length > 0 ? (
        <div role="list" aria-label={images.length === 1 ? "1 image to add" : `${images.length} images to add`} className="flex gap-1.5 overflow-x-auto pb-0.5 pl-1 pr-2 pt-1.5">
          {images.map((img) => {
            const desc = img.sizeLabel ? `${img.name}, ${img.sizeLabel}` : img.name;
            return (
              <div role="listitem" key={img.id} className="relative shrink-0" style={{ animation: "pop-in 160ms cubic-bezier(0.23,1,0.32,1) both" }}>
                <Tooltip content={desc} describe={false}>
                  <img src={img.src} alt={desc} className="size-14 rounded-[8px] object-cover shadow-hairline" draggable={false} />
                </Tooltip>
                {onRemoveImage ? (
                  <Tooltip content={`Remove ${img.name}`} describe={false} className="absolute -right-1.5 -top-1.5">
                    <button
                      type="button"
                      aria-label={`Remove ${img.name}`}
                      disabled={busy}
                      onClick={() => onRemoveImage(img.id)}
                      className="flex size-[18px] items-center justify-center rounded-full bg-[oklch(0.25_0.006_258/0.88)] text-white shadow-btn transition-transform duration-100 hover:bg-[oklch(0.2_0.006_258)] active:scale-90 disabled:pointer-events-none disabled:opacity-50"
                    >
                      <XIcon className="size-2.5" strokeWidth={3} />
                    </button>
                  </Tooltip>
                ) : null}
              </div>
            );
          })}
        </div>
      ) : null}

      <div className="flex items-end gap-1.5">
        {onAttach ? (
          <IconButton
            label="Add images"
            tooltip={attachTooltip ?? "Add images"}
            icon={<PaperclipIcon />}
            onClick={onAttach}
            disabled={attachDisabled || readOnly || busy}
          />
        ) : null}
        <p
          aria-live="polite"
          className={cn(
            "min-h-7 min-w-0 flex-1 self-center py-1 text-[12px] leading-[16px]",
            statusTone === "error" ? "text-danger" : "text-ink-2",
          )}
        >
          {status}
        </p>
        <Button
          variant="primary"
          size="sm"
          icon={<SendIcon strokeWidth={2} />}
          busy={busy}
          disabled={sendDisabled}
          onClick={onSend}
          tooltip={sendTooltip}
          aria-keyshortcuts="Meta+Enter"
          className="shrink-0"
        >
          {sendLabel}
        </Button>
      </div>
    </div>
  );
});
