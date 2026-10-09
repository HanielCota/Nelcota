// How the app's users can sign in, read from the server's settings. Each way
// says whether it is on and, when it is off, which environment variables turn
// it on: they live in the server's .env, so the panel explains instead of
// offering a form that could not save.
import type { SignIn } from '$lib/types'

export type OptionId = 'password' | 'confirmation' | 'recovery' | 'magicLink' | 'google' | 'github'

export interface SignInOption {
  id: OptionId
  on: boolean
  /** Variables that turn it on, shown only while it is off. */
  variables: string[]
}

const MAIL = ['NELCOTA_SMTP_URL', 'NELCOTA_SMTP_FROM', 'NELCOTA_PASSWORD_RECOVERY_URL']
const OAUTH = ['NELCOTA_API_URL', 'NELCOTA_OAUTH_REDIRECT_URLS']

export function signInOptions(settings: SignIn): SignInOption[] {
  const option = (id: OptionId, on: boolean, variables: string[]): SignInOption => ({ id, on, variables: on ? [] : variables })
  // Without SMTP, every email feature needs the mail trio first.
  const mail = (extra: string[]) => (settings.email ? extra : [...MAIL, ...extra])
  return [
    option('password', settings.signup_enabled, ['NELCOTA_SIGNUP_ENABLED=true']),
    option('confirmation', settings.email_confirmation, mail(['NELCOTA_EMAIL_CONFIRMATION_URL'])),
    option('recovery', settings.password_recovery, mail([])),
    option('magicLink', settings.magic_link, mail(['NELCOTA_MAGIC_LINK_URL'])),
    option('google', settings.providers.google, [...OAUTH, 'NELCOTA_OAUTH_GOOGLE_CLIENT_ID', 'NELCOTA_OAUTH_GOOGLE_CLIENT_SECRET']),
    option('github', settings.providers.github, [...OAUTH, 'NELCOTA_OAUTH_GITHUB_CLIENT_ID', 'NELCOTA_OAUTH_GITHUB_CLIENT_SECRET']),
  ]
}

/** Seconds as whole minutes, at least one. */
export const minutes = (seconds: number) => Math.max(1, Math.round(seconds / 60))
