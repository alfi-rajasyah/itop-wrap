import { useCallback } from 'react'
import { Button, Checkbox, ExternalIcon, Field, SegmentedControl, TextArea, TextInput } from '@/components/kit'
import { openUrl, type NewTicketIntent, type NewTicketOpen, type TicketClass } from '@/backend'
import { SnapshotDropdown } from '@/lib/SnapshotDropdown'
import { useModelText } from '@/lib/useModelText'

/**
 * The New ticket fields (NEW-5…13), in Swift's order: type, Organization, Caller, Service and
 * Subcategory side by side, Impact and Urgency side by side, Title, Description, "Assign to
 * me". Required marks, placeholders and enabled states are the snapshot's. Edit ticket
 * (`form.edit`) has no type or "Assign to me", and a formatted description is read-only.
 */
export function NewTicketFields({ form, send }: { form: NewTicketOpen; send: (intent: NewTicketIntent) => Promise<unknown> }) {
  const run = (intent: NewTicketIntent) => void send(intent).catch((error: unknown) => console.error('newTicket intent failed', error))
  const commitTitle = useCallback((title: string) => send({ type: 'setTitle', title }), [send])
  const commitDescription = useCallback((description: string) => send({ type: 'setDescription', description }), [send])
  const [title, setTitle] = useModelText(form.title, commitTitle)
  const [description, setDescription] = useModelText(form.description, commitDescription)

  // No programmatic initial focus: W/NewTicketView.swift sets none (no @FocusState).
  const r = form.required
  const edit = form.edit
  return (
    <div className="flex flex-col gap-1">
      {edit ? null : (
        <SegmentedControl<TicketClass>
          label="Type"
          options={[
            { value: 'UserRequest', label: 'User request' },
            { value: 'Incident', label: 'Incident' },
          ]}
          value={form.ticketClass}
          onChange={(ticketClass) => run({ type: 'setClass', ticketClass })}
          className="mb-2.5"
        />
      )}

      <div>
        <Field label="Organization" required requiredError={r.organization.missing} reserveErrorSpace={false}>
          {({ id }) => (
            <SnapshotDropdown
              id={id}
              aria-labelledby={`${id}-label`}
              aria-required
              dropdown={form.organization}
              onPick={(orgId) => run({ type: 'setOrganization', id: orgId })}
            />
          )}
        </Field>
      </div>

      <Field label="Caller" required requiredError={r.caller.missing} reserveErrorSpace={false} className="mt-2.5">
        {({ id }) => (
          <SnapshotDropdown
            id={id}
            aria-labelledby={`${id}-label`}
            aria-required
            dropdown={form.caller}
            onPick={(callerId) => run({ type: 'setCaller', id: callerId })}
          />
        )}
      </Field>

      <div className="mt-2.5 grid grid-cols-2 gap-2.5">
        <Field label="Service" required requiredError={r.service.missing} reserveErrorSpace={false}>
          {({ id }) => (
            <SnapshotDropdown
              id={id}
              aria-labelledby={`${id}-label`}
              aria-required
              dropdown={form.service}
              onPick={(serviceId) => run({ type: 'setService', id: serviceId })}
            />
          )}
        </Field>
        <Field label="Subcategory" required requiredError={r.subcategory.missing} reserveErrorSpace={false}>
          {({ id }) => (
            <SnapshotDropdown
              id={id}
              aria-label="Service subcategory"
              aria-required
              dropdown={form.subcategory}
              onPick={(subcategoryId) => run({ type: 'setSubcategory', id: subcategoryId })}
            />
          )}
        </Field>
      </div>

      <div className="mt-2.5 grid grid-cols-2 gap-2.5">
        <Field label="Impact" reserveErrorSpace={false}>
          {({ id }) => (
            <SnapshotDropdown id={id} aria-labelledby={`${id}-label`} dropdown={form.impact} onPick={(impact) => run({ type: 'setImpact', id: impact })} />
          )}
        </Field>
        <Field label="Urgency" reserveErrorSpace={false}>
          {({ id }) => (
            <SnapshotDropdown id={id} aria-labelledby={`${id}-label`} dropdown={form.urgency} onPick={(urgency) => run({ type: 'setUrgency', id: urgency })} />
          )}
        </Field>
      </div>

      <Field label="Title" required requiredError={r.title.missing} reserveErrorSpace={false} className="mt-2.5">
        {({ id }) => (
          <TextInput
            id={id}
            aria-required
            placeholder="What's wrong, in a few words"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
        )}
      </Field>

      {edit?.descriptionLocked ? (
        // A formatted description is never rewritten here (it would lose its images).
        <div className="mt-2.5 flex flex-col gap-1.5">
          <span className="text-[12.5px] font-semibold text-ink">Description</span>
          <div className="flex items-start gap-2 rounded-[10px] bg-field px-3 py-2.5 text-[12.5px] leading-snug text-ink-2">
            <p className="min-w-0 flex-1">{edit.descriptionNote}</p>
            <Button size="sm" variant="secondary" icon={<ExternalIcon />} onClick={() => void openUrl(edit.webUrl)}>
              Open in iTop
            </Button>
          </div>
        </div>
      ) : (
        <Field label="Description" required requiredError={r.description.missing} reserveErrorSpace={false} className="mt-2.5">
          {({ id }) => (
            <TextArea
              id={id}
              aria-required
              minHeight={100}
              placeholder="What the caller reports, and anything you've already checked."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          )}
        </Field>
      )}

      {edit ? null : (
        <Checkbox className="mt-3" checked={form.assignToMe} onChange={(assignToMe) => run({ type: 'setAssignToMe', assignToMe })} label="Assign to me" />
      )}

      {form.disabledText ? <p className="mt-2 text-[12px] text-ink-2">{form.disabledText}</p> : null}
    </div>
  )
}

/** The lines under the form: "Fill in the caller first." and the create error. */
export function NewTicketMessages({ form }: { form: NewTicketOpen }) {
  if (!form.missingText && !form.error) return null
  return (
    <span className="flex flex-col gap-0.5">
      {form.missingText ? <span>{form.missingText}</span> : null}
      {form.error ? <span>{form.error}</span> : null}
    </span>
  )
}
