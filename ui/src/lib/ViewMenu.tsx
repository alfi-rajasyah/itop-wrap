import { FilterChip, Menu, ViewIcon, ViewSummary, type MenuEntry } from '@/components/kit'
import { useModel } from '@/backend'

/**
 * The shared "View" menu (LIST-39…43) on the `options` model: range, resolved/closed,
 * sort, page size and "Back to default view". Every title, check and the tooltip come from
 * the snapshot. Used above an organization's and a person's tickets.
 */
export function ViewMenu() {
  const [options, send] = useModel('options')
  if (!options) return null

  const items: MenuEntry[] = [
    { kind: 'section', id: 'range', label: 'Show tickets opened' },
    ...options.rangeChoices.map(
      (c): MenuEntry => ({
        kind: 'radio',
        id: `range-${c.value}`,
        label: c.title,
        checked: c.selected,
        onSelect: () => void send({ type: 'setRange', range: c.value }),
      }),
    ),
    {
      kind: 'checkbox',
      id: 'closed',
      label: 'Include resolved and closed',
      checked: options.includeClosed,
      keepOpen: false,
      onCheckedChange: (checked) => void send({ type: 'setIncludeClosed', includeClosed: checked }),
    },
    { kind: 'section', id: 'sort', label: 'Sort inside each status group' },
    ...options.sortChoices.map(
      (c): MenuEntry => ({
        kind: 'radio',
        id: `sort-${c.value}`,
        label: c.title,
        checked: c.selected,
        onSelect: () => void send({ type: 'setSort', sort: c.value }),
      }),
    ),
    { kind: 'section', id: 'size', label: 'Per page' },
    ...options.pageSizeChoices.map(
      (c): MenuEntry => ({
        kind: 'radio',
        id: `size-${c.value ?? 'all'}`,
        label: c.title,
        checked: c.selected,
        onSelect: () => void send({ type: 'setPageSize', pageSize: c.value }),
      }),
    ),
    ...(options.isDefault
      ? []
      : ([
          { kind: 'separator', id: 'sep' },
          { id: 'reset', label: 'Back to default view', onSelect: () => void send({ type: 'reset' }) },
        ] satisfies MenuEntry[])),
  ]

  return (
    <Menu
      label="View options"
      items={items}
      placement="bottom-end"
      minWidth={230}
      trigger={(props) => (
        <FilterChip
          {...props}
          icon={<ViewIcon />}
          label="View"
          active={!options.isDefault}
          tooltip={options.help}
          aria-label={options.accessibilityLabel}
        />
      )}
    />
  )
}

/** The quiet line under the controls saying how the view differs from the default (LIST-43). */
export function ViewOptionsSummary({ className }: { className?: string }) {
  const [options] = useModel('options')
  return <ViewSummary className={className}>{options?.summary ?? null}</ViewSummary>
}
