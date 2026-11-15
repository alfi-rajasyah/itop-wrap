import { useCallback, useEffect, useRef } from 'react'
import { Checkbox, Field, SegmentedControl, TextArea, TextInput } from '@/components/kit'
import type { NewTicketIntent, NewTicketOpen, TicketClass } from '@/backend'
import { SnapshotDropdown } from '@/lib/SnapshotDropdown'
import { useModelText } from '@/lib/useModelText'

/**
 * The New ticket fields (NEW-5…13), in Swift's order: type, Organization, Caller, Service and
 * Subcategory side by side, Impact and Urgency side by side, Title, Description, "Assign to
 * me". Required marks, placeholders and enabled states are the snapshot's.
 */
export function NewTicketFields({ form, send }: { form: NewTicketOpen; send: (intent: NewTicketIntent) => Promise<unknown> }) {
  const run = (intent: NewTicketIntent) => void send(intent).catch((error: unknown) => console.error('newTicket intent failed', error))
  const commitTitle = useCallback((title: string) => send({ type: 'setTitle', title }), [send])
  const commitDescription = useCallback((description: string) => send({ type: 'setDescription', description }), [send])
  const [title, setTitle] = useModelText(form.title, commitTitle)
  const [description, setDescription] = useModelText(form.description, commitDescription)

  // Initial focus: the first field to fill once the organizations have loaded.
  const orgRef = useRef<HTMLDivElement>(null)
  const titleRef = useRef<HTMLInputElement>(null)
  const focused = useRef(false)
  const orgReady = form.organization.enabled
  const orgChosen = form.organization.selectedId !== null
  useEffect(() => {
    if (focused.current || !orgReady) return
    focused.current = true
    const active = document.activeElement
    const untouched = !active || active === document.body || active.getAttribute('role') === 'dialog' || active.getAttribute('role') === 'radio'
    if (!untouched) return
    if (orgChosen) titleRef.current?.focus()
    else orgRef.current?.querySelector<HTMLInputElement>('input')?.focus()
  }, [orgReady, orgChosen])

  const r = form.required
  return (
    <div className="flex flex-col gap-1">
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

      <div ref={orgRef}>
        <Field label="Organization" required requiredError={r.organization.missing} reserveErrorSpace={false}>
          {({ id }) => (
            <SnapshotDropdown
              id={id}
              aria-labelledby={`${id}-label`}
              aria-required
              invalid={r.organization.missing}
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
            invalid={r.caller.missing}
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
              invalid={r.service.missing}
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
              invalid={r.subcategory.missing}
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
            ref={titleRef}
            id={id}
            aria-required
            invalid={r.title.missing}
            placeholder="What's wrong, in a few words"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
        )}
      </Field>

      <Field label="Description" required requiredError={r.description.missing} reserveErrorSpace={false} className="mt-2.5">
        {({ id }) => (
          <TextArea
            id={id}
            aria-required
            invalid={r.description.missing}
            minHeight={100}
            placeholder="What the caller reports, and anything you've already checked."
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
        )}
      </Field>

      <Checkbox className="mt-3" checked={form.assignToMe} onChange={(assignToMe) => run({ type: 'setAssignToMe', assignToMe })} label="Assign to me" />

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
