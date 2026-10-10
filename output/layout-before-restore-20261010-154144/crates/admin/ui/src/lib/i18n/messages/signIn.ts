// Sign-in page: how the app's users get in, read from the server's settings.
import type { Messages } from '../index.svelte'

const ptBR = {
  examples: 'Ver exemplos de login',
  title: 'Login dos usuários',
  description: 'Como as pessoas entram no seu app.',
  fromEnv: 'Vem do .env do servidor; mudanças pedem reinício.',
  on: 'Ligado',
  off: 'Desligado',
  summary: { one: '{on} de {count} forma de entrar ligada', other: '{on} de {count} formas de entrar ligadas' },
  howTo: 'Como ligar',
  envHint: 'Adicione ao .env do projeto, preencha os valores e reinicie (nelcota up).',
  copyEnv: 'Copiar variáveis',
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
    access: { label: 'Duração do acesso', unit: { one: 'minuto', other: 'minutos' }, hint: 'O SDK renova sozinho antes de acabar.' },
    refresh: { label: 'Sessão parada', unit: { one: 'dia', other: 'dias' }, hint: 'Depois disso sem usar o app, é preciso entrar de novo.' },
    rateLimit: { label: 'Tentativas de login', unit: { one: 'por minuto', other: 'por minuto' }, hint: 'Por endereço IP; o excesso é recusado.' },
  },
}

const en: Messages<typeof ptBR> = {
  examples: 'View sign-in examples',
  title: 'User sign-in',
  description: 'How people get into your app.',
  fromEnv: "Read from the server's .env; changes need a restart.",
  on: 'On',
  off: 'Off',
  summary: { one: '{on} of {count} way to sign in is on', other: '{on} of {count} ways to sign in are on' },
  howTo: 'How to turn on',
  envHint: "Add to the project's .env, fill in the values and restart (nelcota up).",
  copyEnv: 'Copy variables',
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
    access: { label: 'Access lasts', unit: { one: 'minute', other: 'minutes' }, hint: 'The SDK refreshes it before it runs out.' },
    refresh: { label: 'Idle session', unit: { one: 'day', other: 'days' }, hint: 'After that long without using the app, people sign in again.' },
    rateLimit: { label: 'Sign-in attempts', unit: { one: 'per minute', other: 'per minute' }, hint: 'Per IP address; the rest are refused.' },
  },
}

export default { 'pt-BR': ptBR, en }
