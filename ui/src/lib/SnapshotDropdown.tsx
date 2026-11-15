import { DropdownField, type DropdownFieldProps } from '@/components/kit'
import type { Dropdown } from '@/backend'

/**
 * A snapshot `Dropdown` as the kit's picker (KEY-11…17): items, selection, placeholder,
 * enabled and, for Organization and Caller, the search prompt and no-match text.
 */
export function SnapshotDropdown({
  dropdown,
  onPick,
  ...rest
}: {
  dropdown: Dropdown
  onPick: (id: string) => void
} & Pick<DropdownFieldProps, 'id' | 'invalid' | 'aria-label' | 'aria-labelledby' | 'aria-describedby' | 'aria-required' | 'className'>) {
  return (
    <DropdownField
      items={dropdown.items.map((i) => ({ value: i.id, label: i.label }))}
      value={dropdown.selectedId}
      onChange={onPick}
      placeholder={dropdown.placeholder}
      disabled={!dropdown.enabled}
      searchable={dropdown.search !== null}
      searchPrompt={dropdown.search?.prompt}
      noMatchText={dropdown.search?.noMatch}
      {...rest}
    />
  )
}
