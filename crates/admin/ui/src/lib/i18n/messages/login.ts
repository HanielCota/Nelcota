// Login screen.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Entrar no Nelcota',
  projectLabel: 'Projeto:',
  projectPanel: 'Painel do projeto',
  adminPanel: 'Painel administrativo',
  email: 'Email',
  emailPlaceholder: 'voce@empresa.com',
  password: 'Senha',
  signIn: 'Entrar',
  signingIn: 'Entrando…',
  capsLock: 'Caps Lock ativado.',
  restricted: 'Acesso exclusivo para administradores.',
  sessionExpired: 'Sua sessão expirou. Entre de novo para continuar.',
}

const en: Messages<typeof ptBR> = {
  title: 'Sign in to Nelcota',
  projectLabel: 'Project:',
  projectPanel: 'Panel of project',
  adminPanel: 'Admin panel',
  email: 'Email',
  emailPlaceholder: 'you@company.com',
  password: 'Password',
  signIn: 'Sign in',
  signingIn: 'Signing in…',
  capsLock: 'Caps Lock is on.',
  restricted: 'Administrator access only.',
  sessionExpired: 'Your session expired. Sign in again to continue.',
}

export default { 'pt-BR': ptBR, en }
