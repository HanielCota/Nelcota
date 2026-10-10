// Login screen.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Entrar no Nelcota',
  projectLabel: 'Projeto:',
  adminPanel: 'Painel administrativo',
  email: 'Email',
  emailPlaceholder: 'voce@empresa.com',
  password: 'Senha',
  signIn: 'Entrar',
  signingIn: 'Entrando…',
  capsLock: 'Caps Lock ativado.',
  restricted: 'Acesso exclusivo para administradores.',
}

const en: Messages<typeof ptBR> = {
  title: 'Sign in to Nelcota',
  projectLabel: 'Project:',
  adminPanel: 'Admin panel',
  email: 'Email',
  emailPlaceholder: 'you@company.com',
  password: 'Password',
  signIn: 'Sign in',
  signingIn: 'Signing in…',
  capsLock: 'Caps Lock is on.',
  restricted: 'Administrator access only.',
}

export default { 'pt-BR': ptBR, en }
