// Login screen.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Entrar no Nelcota',
  projectPanel: 'Painel do projeto',
  adminPanel: 'Painel administrativo',
  email: 'Email',
  password: 'Senha',
  signIn: 'Entrar',
  signingIn: 'Entrando…',
  restricted: 'Acesso restrito aos administradores deste host.',
}

const en: Messages<typeof ptBR> = {
  title: 'Sign in to Nelcota',
  projectPanel: 'Panel of project',
  adminPanel: 'Admin panel',
  email: 'Email',
  password: 'Password',
  signIn: 'Sign in',
  signingIn: 'Signing in…',
  restricted: 'Access restricted to the administrators of this host.',
}

export default { 'pt-BR': ptBR, en }
