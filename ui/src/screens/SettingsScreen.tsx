import { useModel } from '@/backend'
import { Checkbox } from '@/components/kit'

/*
 * Settings (iTop Wrap ▸ Settings…, ⌘,, or the menu bar panel's gear; iTop Wrap addition): which
 * icons the app shows. Rust keeps at least one on, so the toggle that would leave nothing to
 * click is disabled, with the reason under it.
 */
export default function SettingsScreen() {
  const [settings, send] = useModel('settings')
  if (!settings) return null
  const locked = !settings.canHideDock || !settings.canHideMenuBarIcon
  return (
    <div className="flex flex-col bg-surface px-5 pb-5 pt-4 text-ink">
      <h1 className="text-[15px] font-semibold leading-snug">App icons</h1>
      <p className="mt-0.5 text-[13px] leading-snug text-ink-2">Choose where iTop Wrap shows up.</p>
      <div className="mt-4 flex flex-col gap-3">
        <Option
          label="Show in the Dock"
          detail="Hidden, iTop Wrap lives in the menu bar while you're signed in."
          checked={settings.showInDock}
          disabled={!settings.canHideDock}
          onChange={(on) => void send({ type: 'setShowInDock', on })}
        />
        <Option
          label="Show in the menu bar"
          detail="Your tickets and the timer, one click away."
          checked={settings.showMenuBarIcon}
          disabled={!settings.canHideMenuBarIcon}
          onChange={(on) => void send({ type: 'setShowMenuBarIcon', on })}
        />
      </div>
      <p className="mt-4 min-h-[16px] text-[12px] leading-snug text-ink-2" aria-live="polite">
        {locked ? 'One of the two stays on, so you can always get back to iTop Wrap.' : null}
      </p>
    </div>
  )
}

function Option({
  label,
  detail,
  checked,
  disabled,
  onChange,
}: {
  label: string
  detail: string
  checked: boolean
  disabled: boolean
  onChange: (on: boolean) => void
}) {
  return (
    <div className="flex flex-col gap-0.5">
      <Checkbox label={label} checked={checked} disabled={disabled} onChange={onChange} />
      <p className="pl-6 text-[12px] leading-snug text-ink-2">{detail}</p>
    </div>
  )
}
