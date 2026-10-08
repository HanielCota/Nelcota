// App shell: sidebar, topbar, account menu, page names and tab titles.
import type { Messages } from '../index.svelte'

const ptBR = {
  pages: {
    overview: 'Visão geral',
    tables: 'Tabelas',
    structure: 'Estrutura',
    sql: 'Editor SQL',
    migrations: 'Migrações',
    users: 'Usuários',
    storage: 'Armazenamento',
    policies: 'Políticas de acesso',
    connect: 'API',
    projects: 'Projetos',
    notFound: 'Página não encontrada',
    signIn: 'Entrar',
  },
  groups: {
    database: 'Banco de dados',
    access: 'Acesso',
    integration: 'Integração',
  },
  nav: {
    main: 'Navegação principal',
    breadcrumb: 'Trilha',
    menu: 'Menu',
    docs: 'Docs (OpenAPI)',
    collapse: 'Recolher barra lateral',
    pin: 'Fixar barra lateral aberta',
  },
  topbar: {
    search: 'Buscar…',
    searchLabel: 'Buscar',
    connect: 'Conectar',
  },
  account: {
    label: 'Conta',
    signedInAs: 'Conectado como',
    addPhoto: 'Adicionar foto…',
    changePhoto: 'Alterar foto…',
    lightTheme: 'Tema claro',
    darkTheme: 'Tema escuro',
    language: 'Idioma',
    signOut: 'Sair',
  },
  languages: {
    'pt-BR': 'Português',
    en: 'English',
  },
  projects: {
    switch: 'Trocar de projeto',
    heading: 'Projetos',
    separateLogin: 'Projetos (cada um pede o próprio login)',
    all: 'Todos os projetos',
  },
  app: {
    loading: 'Carregando painel…',
    skipToContent: 'Pular para o conteúdo',
  },
}

const en: Messages<typeof ptBR> = {
  pages: {
    overview: 'Overview',
    tables: 'Tables',
    structure: 'Structure',
    sql: 'SQL editor',
    migrations: 'Migrations',
    users: 'Users',
    storage: 'Storage',
    policies: 'Policies',
    connect: 'API',
    projects: 'Projects',
    notFound: 'Page not found',
    signIn: 'Sign in',
  },
  groups: {
    database: 'Database',
    access: 'Access',
    integration: 'Integration',
  },
  nav: {
    main: 'Main navigation',
    breadcrumb: 'Breadcrumb',
    menu: 'Menu',
    docs: 'Docs (OpenAPI)',
    collapse: 'Collapse sidebar',
    pin: 'Keep sidebar open',
  },
  topbar: {
    search: 'Search…',
    searchLabel: 'Search',
    connect: 'Connect',
  },
  account: {
    label: 'Account',
    signedInAs: 'Signed in as',
    addPhoto: 'Add photo…',
    changePhoto: 'Change photo…',
    lightTheme: 'Light theme',
    darkTheme: 'Dark theme',
    language: 'Language',
    signOut: 'Sign out',
  },
  languages: {
    'pt-BR': 'Português',
    en: 'English',
  },
  projects: {
    switch: 'Switch project',
    heading: 'Projects',
    separateLogin: 'Projects (each asks for its own login)',
    all: 'All projects',
  },
  app: {
    loading: 'Loading panel…',
    skipToContent: 'Skip to content',
  },
}

export default { 'pt-BR': ptBR, en }
