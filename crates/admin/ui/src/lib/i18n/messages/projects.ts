// Projects page (every project on this host).
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Projetos',
  sso: 'Login único: os painéis abrem sem pedir senha.',
  separateLogin: 'Cada painel pede o próprio login.',
  refreshing: 'Atualizando…',
  current: 'este painel',
  up: 'No ar',
  down: 'Fora do ar',
  empty: 'Nenhum projeto encontrado',
  emptyDescription: 'Crie um projeto com nelcota init e atualize esta lista.',
  copyUrl: 'Copiar endereço de {name}',
  open: 'Abrir painel',
  newProject: {
    title: 'Novo projeto',
    subdomain: 'Num subdomínio deste servidor:',
    subdomainCommand: 'nelcota init --project nome',
    domain: 'Ou num domínio próprio:',
    domainCommand: 'nelcota init api.dominio.com',
  },
}

const en: Messages<typeof ptBR> = {
  title: 'Projects',
  sso: 'Single sign-on: panels open without asking for a password.',
  separateLogin: 'Each panel asks for its own login.',
  refreshing: 'Refreshing…',
  current: 'this panel',
  up: 'Up',
  down: 'Down',
  empty: 'No projects found',
  emptyDescription: 'Create a project with nelcota init and refresh this list.',
  copyUrl: 'Copy address for {name}',
  open: 'Open panel',
  newProject: {
    title: 'New project',
    subdomain: 'On a subdomain of this server:',
    subdomainCommand: 'nelcota init --project name',
    domain: 'Or on a domain of its own:',
    domainCommand: 'nelcota init api.domain.com',
  },
}

export default { 'pt-BR': ptBR, en }
