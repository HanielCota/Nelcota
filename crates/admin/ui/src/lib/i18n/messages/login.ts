// Login screen.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Entrar no Nelcota',
  project: 'Projeto',
  subtitle: 'Use a conta de administrador deste servidor.',
  email: 'Email',
  emailPlaceholder: 'voce@empresa.com',
  password: 'Senha',
  signIn: 'Entrar',
  signingIn: 'Entrando…',
  capsLock: 'Caps Lock ativado.',
  emailRequired: 'Digite seu email.',
  passwordRequired: 'Digite sua senha.',
  restricted: 'Acesso exclusivo para administradores.',
  sessionExpired: 'Sua sessão expirou. Entre de novo para continuar.',
}

const en: Messages<typeof ptBR> = {
  title: 'Sign in to Nelcota',
  project: 'Project',
  subtitle: "Use this server's admin account.",
  email: 'Email',
  emailPlaceholder: 'you@company.com',
  password: 'Password',
  signIn: 'Sign in',
  signingIn: 'Signing in…',
  capsLock: 'Caps Lock is on.',
  emailRequired: 'Enter your email.',
  passwordRequired: 'Enter your password.',
  restricted: 'Administrator access only.',
  sessionExpired: 'Your session expired. Sign in again to continue.',
}

export default { 'pt-BR': ptBR, en }
