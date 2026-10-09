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
    userSignIn: 'Login dos usuários',
  },
  // Short page names for the top pills; page titles keep the full names.
  pills: {
    overview: 'Visão geral',
    tables: 'Tabelas',
    sql: 'SQL',
    migrations: 'Migrações',
    storage: 'Arquivos',
    users: 'Usuários',
    policies: 'Acesso',
    connect: 'API',
  },
  nav: {
    main: 'Navegação principal',
    menu: 'Menu',
  },
  topbar: {
    searchLabel: 'Buscar',
  },
  account: {
    label: 'Conta',
    role: 'Administrador',
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
    userSignIn: 'User sign-in',
  },
  pills: {
    overview: 'Overview',
    tables: 'Tables',
    sql: 'SQL',
    migrations: 'Migrations',
    storage: 'Files',
    users: 'Users',
    policies: 'Access',
    connect: 'API',
  },
  nav: {
    main: 'Main navigation',
    menu: 'Menu',
  },
  topbar: {
    searchLabel: 'Search',
  },
  account: {
    label: 'Account',
    role: 'Administrator',
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
