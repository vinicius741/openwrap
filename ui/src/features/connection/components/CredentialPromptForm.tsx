import { useEffect, useState } from 'react'
import type { CredentialPrompt } from '../../../types/ipc'

interface CredentialPromptFormProps {
  prompt: CredentialPrompt | null
  onSubmit: (credentials: { username: string; password: string; rememberInKeychain: boolean }) => void
}

export function CredentialPromptForm({ prompt, onSubmit }: CredentialPromptFormProps) {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [remember, setRemember] = useState(true)

  useEffect(() => {
    if (!prompt) {
      return
    }

    setUsername(prompt.saved_username ?? '')
    setPassword('')
    setRemember(true)
  }, [prompt])

  if (!prompt) {
    return null
  }

  return (
    <form
      className="credential-form"
      onSubmit={(event) => {
        event.preventDefault()
        void onSubmit({
          username,
          password,
          rememberInKeychain: remember,
        })
      }}
    >
      <h4>Credentials required</h4>
      <label>
        Username
        <input value={username} onChange={(event) => setUsername(event.target.value)} />
      </label>
      <label>
        Password
        <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} />
      </label>
      <label className="checkbox-row">
        <input checked={remember} onChange={(event) => setRemember(event.target.checked)} type="checkbox" />
        Remember username
      </label>
      <button className="action-button action-primary" type="submit">
        Continue
      </button>
    </form>
  )
}
