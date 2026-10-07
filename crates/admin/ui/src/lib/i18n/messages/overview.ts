// Overview page and the 404 page.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Visão geral',
  description: 'Schema {schema} exposto pela API REST.',
  stats: {
    tables: 'Tabelas',
    users: 'Usuários',
    policies: 'Policies',
    functions: 'Funções',
  },
  exposed: {
    title: 'Expostas sem RLS: {tables}',
    before: 'Quem tem GRANT nessas tabelas lê e altera todas as linhas. Ative com',
    after: 'e crie policies, ou revogue os grants.',
  },
  tables: {
    heading: 'Tabelas',
    openEditor: 'Abrir editor',
    empty: 'Nenhuma tabela ainda',
    emptyBefore: 'Crie com',
    emptyMiddle: 'ou no',
    table: 'Tabela',
    rows: 'Linhas',
    view: 'view',
  },
  notFound: {
    title: 'Página não encontrada',
    description: 'O endereço não existe neste painel.',
    back: 'Voltar à visão geral',
  },
}

const en: Messages<typeof ptBR> = {
  title: 'Overview',
  description: 'Schema {schema} exposed by the REST API.',
  stats: {
    tables: 'Tables',
    users: 'Users',
    policies: 'Policies',
    functions: 'Functions',
  },
  exposed: {
    title: 'Exposed without RLS: {tables}',
    before: 'Anyone with a GRANT on these tables reads and changes every row. Turn it on with',
    after: 'and create policies, or revoke the grants.',
  },
  tables: {
    heading: 'Tables',
    openEditor: 'Open editor',
    empty: 'No tables yet',
    emptyBefore: 'Create them with',
    emptyMiddle: 'or in the',
    table: 'Table',
    rows: 'Rows',
    view: 'view',
  },
  notFound: {
    title: 'Page not found',
    description: 'This address does not exist in this panel.',
    back: 'Back to the overview',
  },
}

export default { 'pt-BR': ptBR, en }
