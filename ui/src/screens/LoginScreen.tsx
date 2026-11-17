import { useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react'
import { useModel, type LoginForm, type LoginMessage, type Send } from '@/backend'
import { Button, Spinner, TextInput } from '@/components/kit'
import { useWindowKeyDown } from '@/lib/keys'
import { useModelText } from '@/lib/useModelText'

/*
 * Everything the main window shows without a session (AUTH-1…19, KEY-7):
 * restoring ("Signing in…", 480×320), "Can't reach iTop" (480×320) and the sign-in form
 * (480×520). Rust sizes the window; here each state fills a box of that size, centred, so the
 * browser mock shows the same thing.
 */
export default function LoginScreen() {
  const [session, send] = useModel('session')
  if (!session || session.type === 'signedIn') return null
  if (session.type === 'restoring') return <Restoring />
  if (session.type === 'unreachable') return <Unreachable message={session.message} send={send} />
  return <SignInForm form={session.form} send={send} />
}

type SessionSend = Send<'session'>

/** The window's top strip: draggable, clear of the traffic lights. */
function WindowFrame({ height, children }: { height: 320 | 520; children: ReactNode }) {
  return (
    <div className="flex h-screen w-full items-center justify-center bg-surface text-ink">
      <div className="relative flex h-full w-full max-w-[480px] flex-col" style={{ maxHeight: height }}>
        <div data-tauri-drag-region className="h-7 shrink-0" />
        {children}
      </div>
    </div>
  )
}

function Restoring() {
  return (
    <WindowFrame height={320}>
      <div role="status" className="-mt-7 flex flex-1 items-center justify-center gap-2 text-[13px] text-ink-2">
        <Spinner size={14} />
        Signing in…
      </div>
    </WindowFrame>
  )
}

function Unreachable({ message, send }: { message: string; send: SessionSend }) {
  // Return triggers "Try again", the default button (AUTH-5, KEY-7), unless focus is on another control.
  useWindowKeyDown((e) => {
    if (e.key !== 'Enter' || e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return
    if (e.target instanceof Element && e.target.closest('button, a, input, textarea')) return
    e.preventDefault()
    void send({ type: 'retryRestore' })
  })
  return (
    <WindowFrame height={320}>
      <div className="flex flex-1 flex-col gap-3.5 px-12 pb-8 pt-2">
        <h1 className="text-[20px] font-semibold leading-tight">Can't reach iTop</h1>
        <p className="text-[13px] leading-snug text-ink-2">{message}</p>
        <div className="flex-1" />
        <Button variant="primary" size="block" onClick={() => void send({ type: 'retryRestore' })}>
          Try again
        </Button>
        <button
          type="button"
          onClick={() => void send({ type: 'signInWithAnotherAccount' })}
          className="mx-auto rounded-[6px] px-1.5 text-[13px] text-ink underline-offset-2 hover:underline"
        >
          Sign in with another account
        </button>
      </div>
    </WindowFrame>
  )
}

type Focusable = 'server' | 'user' | 'password'

function SignInForm({ form, send }: { form: LoginForm; send: SessionSend }) {
  const [server, setServer] = useModelText(form.server, (server) => send({ type: 'setServer', server }))
  const [user, setUser] = useModelText(form.user, (user) => send({ type: 'setUser', user }))
  const [password, setPassword] = useState('')
  const submitting = useRef(false)
  const refs = {
    server: useRef<HTMLInputElement>(null),
    user: useRef<HTMLInputElement>(null),
    password: useRef<HTMLInputElement>(null),
  }
  const focus = (field: Focusable) => refs[field].current?.focus()

  // AUTH-12: where the cursor starts is decided in Rust.
  const initialFocus = useRef(form.initialFocus)
  useEffect(() => refs[initialFocus.current].current?.focus(), []) // eslint-disable-line react-hooks/exhaustive-deps

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    // AUTH-14: a second submit while signing in is ignored.
    if (submitting.current || form.signingIn) return
    submitting.current = true
    try {
      const result = await send({ type: 'signIn', password })
      if (result.ok) setPassword('')
      else if (result.focus) focus(result.focus)
    } finally {
      submitting.current = false
    }
  }

  return (
    <WindowFrame height={520}>
      <form noValidate onSubmit={submit} className="flex flex-1 flex-col gap-3.5 px-12 pb-8 pt-2">
        <div className="flex flex-col gap-1 pb-1.5">
          <h1 className="text-[20px] font-semibold leading-tight">Sign in to iTop</h1>
          <p className="text-[13px] leading-snug text-ink-2">Use your iTop account. Your password isn't saved: iTop Wrap signs you out after 12 hours without use, and when it quits.</p>
        </div>

        <LoginField label="Server URL" id="login-server">
          <TextInput
            ref={refs.server}
            id="login-server"
            type="text"
            inputMode="url"
            autoComplete="url"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            placeholder="https://itop.example.com"
            value={server}
            onChange={(e) => setServer(e.target.value)}
            aria-describedby={form.httpWarning ? 'login-http-warning' : undefined}
          />
          {form.httpWarning ? (
            <p id="login-http-warning" className="text-[12px] leading-snug text-danger">
              This address isn't encrypted (http). Your password would be sent in plain text. Use https if your server supports it.
            </p>
          ) : null}
        </LoginField>

        <LoginField label="Username" id="login-user">
          <TextInput
            ref={refs.user}
            id="login-user"
            type="text"
            autoComplete="username"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            value={user}
            onChange={(e) => setUser(e.target.value)}
          />
        </LoginField>

        <LoginField label="Password" id="login-password">
          <TextInput
            ref={refs.password}
            id="login-password"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </LoginField>

        <div className="flex-1" />
        <div aria-live="polite">{form.message ? <Message message={form.message} /> : null}</div>
        <Button type="submit" variant="primary" size="block" busy={form.signingIn} aria-busy={form.signingIn || undefined}>
          {form.signingIn ? 'Signing in…' : 'Sign in to iTop'}
        </Button>
      </form>
    </WindowFrame>
  )
}

function LoginField({ label, id, children }: { label: string; id: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-[12px] font-medium text-ink">
        {label}
      </label>
      {children}
    </div>
  )
}

/** AUTH-15…19. Rust has already picked the one message to show. */
function Message({ message }: { message: LoginMessage }) {
  if (message.type === 'restRefused')
    return (
      <p className="rounded-[8px] border border-danger p-2.5 text-[12px] leading-snug text-ink">
        <strong className="font-semibold text-danger">{message.bold}</strong>
        {message.rest}
      </p>
    )
  return <p className="text-[12px] leading-snug text-danger">{message.text}</p>
}
