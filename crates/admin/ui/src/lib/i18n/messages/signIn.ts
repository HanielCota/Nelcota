// Sign-in page: how the app's users get in, read from the server's settings.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Login dos usuários',
  description: 'Como as pessoas entram no seu app.',
  fromEnv:
    'Estas opções vêm das variáveis de ambiente do servidor: para mudar, edite o .env do projeto e reinicie (nelcota up).',
  on: 'Ligado',
  off: 'Desligado',
  turnOn: 'Para ligar:',
  options: {
    password: { title: 'Cadastro com email e senha', text: 'Qualquer pessoa cria uma conta pelo app. Desligado, só o painel cria usuários.' },
    confirmation: { title: 'Confirmação de email', text: 'A conta só entra depois de clicar no link enviado por email.' },
    recovery: { title: 'Recuperação de senha', text: 'A pessoa recebe um link por email para escolher uma senha nova.' },
    magicLink: { title: 'Link mágico', text: 'Entrar sem senha, por um link enviado por email.' },
    google: { title: 'Google', text: 'Entrar com a conta Google.' },
    github: { title: 'GitHub', text: 'Entrar com a conta GitHub.' },
  },
  providers: {
    title: 'Google e GitHub',
    callback: 'Endereço de retorno para cadastrar no Google ou no GitHub',
    copyCallback: 'Copiar endereço de retorno',
    pages: 'Páginas do app para onde o login pode voltar',
    noPages: 'Nenhuma página cadastrada (NELCOTA_OAUTH_REDIRECT_URLS).',
  },
  session: {
    title: 'Sessão',
    access: { one: 'O acesso dura {count} minuto e o SDK renova sozinho.', other: 'O acesso dura {count} minutos e o SDK renova sozinho.' },
    refresh: { one: 'Quem não usa o app por {count} dia precisa entrar de novo.', other: 'Quem não usa o app por {count} dias precisa entrar de novo.' },
    rateLimit: { one: 'Até {count} tentativa de login por minuto por endereço IP.', other: 'Até {count} tentativas de login por minuto por endereço IP.' },
  },
}

const en: Messages<typeof ptBR> = {
  title: 'User sign-in',
  description: 'How people get into your app.',
  fromEnv:
    "These options come from the server's environment variables: to change them, edit the project's .env and restart (nelcota up).",
  on: 'On',
  off: 'Off',
  turnOn: 'To turn it on:',
  options: {
    password: { title: 'Sign-up with email and password', text: 'Anyone can create an account from the app. When off, only the panel creates users.' },
    confirmation: { title: 'Email confirmation', text: 'An account can only sign in after clicking the link sent by email.' },
    recovery: { title: 'Password recovery', text: 'People get a link by email to choose a new password.' },
    magicLink: { title: 'Magic link', text: 'Sign in without a password, through a link sent by email.' },
    google: { title: 'Google', text: 'Sign in with a Google account.' },
    github: { title: 'GitHub', text: 'Sign in with a GitHub account.' },
  },
  providers: {
    title: 'Google and GitHub',
    callback: 'Callback address to register with Google or GitHub',
    copyCallback: 'Copy callback address',
    pages: 'App pages the sign-in may return to',
    noPages: 'No page registered (NELCOTA_OAUTH_REDIRECT_URLS).',
  },
  session: {
    title: 'Session',
    access: { one: 'Access lasts {count} minute and the SDK refreshes it by itself.', other: 'Access lasts {count} minutes and the SDK refreshes it by itself.' },
    refresh: { one: 'Someone who does not use the app for {count} day must sign in again.', other: 'Someone who does not use the app for {count} days must sign in again.' },
    rateLimit: { one: 'Up to {count} sign-in attempt per minute per IP address.', other: 'Up to {count} sign-in attempts per minute per IP address.' },
  },
}

export default { 'pt-BR': ptBR, en }
