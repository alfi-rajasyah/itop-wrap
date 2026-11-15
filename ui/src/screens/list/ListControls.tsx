import { useModel, type OrgFilter, type Send } from '@/backend'
import { BuildingIcon, FilterChip, Menu, ViewIcon, ViewSummary, type MenuEntry } from '@/components/kit'

/*
 * Above the list, not scrolling (LIST-45): the org filter (My tickets, LIST-33…36), the shared
 * View menu (LIST-39…41) and the summary of what differs from the default (LIST-42, LIST-43).
 */

export function OrgFilterMenu({ filter, send }: { filter: OrgFilter; send: Send<'list:mine'> }) {
  const items: MenuEntry[] = [
    {
      kind: 'radio',
      id: 'all',
      label: 'All organizations',
      checked: filter.selected === null,
      onSelect: () => void send({ type: 'clearOrgFilter' }),
    },
    { kind: 'separator', id: 'sep' },
    ...filter.choices.map(
      (choice): MenuEntry => ({
        kind: 'radio',
        id: `org-${choice.name}`,
        label: choice.name,
        checked: choice.selected,
        onSelect: () => void send({ type: 'toggleOrg', name: choice.name }),
      }),
    ),
  ]
  return (
    <Menu
      label="Organizations"
      items={items}
      minWidth={220}
      trigger={({ ref, ...props }) => (
        <FilterChip
          ref={ref}
          icon={<BuildingIcon />}
          label={filter.label}
          active={filter.selected !== null}
          tooltip={filter.help}
          aria-label={filter.accessibilityLabel}
          maxWidth={200}
          {...props}
        />
      )}
    />
  )
}

export function ViewMenu() {
  const [options, send] = useModel('options')
  if (!options) return null
  const items: MenuEntry[] = [
    { kind: 'section', id: 's-range', label: 'Show tickets opened' },
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
      onCheckedChange: (includeClosed) => void send({ type: 'setIncludeClosed', includeClosed }),
    },
    { kind: 'separator', id: 'sep-1' },
    { kind: 'section', id: 's-sort', label: 'Sort inside each status group' },
    ...options.sortChoices.map(
      (c): MenuEntry => ({
        kind: 'radio',
        id: `sort-${c.value}`,
        label: c.title,
        checked: c.selected,
        onSelect: () => void send({ type: 'setSort', sort: c.value }),
      }),
    ),
    { kind: 'separator', id: 'sep-2' },
    { kind: 'section', id: 's-page', label: 'Per page' },
    ...options.pageSizeChoices.map(
      (c): MenuEntry => ({
        kind: 'radio',
        id: `page-${c.value ?? 'all'}`,
        label: c.title,
        checked: c.selected,
        onSelect: () => void send({ type: 'setPageSize', pageSize: c.value }),
      }),
    ),
    ...(options.isDefault
      ? []
      : ([
          { kind: 'separator', id: 'sep-3' },
          { id: 'reset', label: 'Back to default view', onSelect: () => void send({ type: 'reset' }) },
        ] satisfies MenuEntry[])),
  ]
  return (
    <Menu
      label="View options"
      items={items}
      minWidth={250}
      trigger={({ ref, ...props }) => (
        <FilterChip
          ref={ref}
          icon={<ViewIcon />}
          label="View"
          active={!options.isDefault}
          tooltip={options.help}
          aria-label={options.accessibilityLabel}
          {...props}
        />
      )}
    />
  )
}

export function ListControls({ orgFilter, send }: { orgFilter: OrgFilter; send: Send<'list:mine'> }) {
  const [options] = useModel('options')
  return (
    <div className="flex shrink-0 flex-col gap-1.5 px-3 pb-2">
      <div className="flex min-w-0 items-center gap-1.5">
        {orgFilter.visible ? <OrgFilterMenu filter={orgFilter} send={send} /> : null}
        <ViewMenu />
      </div>
      <ViewSummary className="px-0.5">{options?.summary}</ViewSummary>
    </div>
  )
}
