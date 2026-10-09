// Overview page and the 404 page.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Visão geral',
  description: 'Seu projeto está no ar.',
  steps: {
    title: 'Próximos passos',
    progress: '{done} de {total}',
    hide: 'Ocultar',
    done: 'Concluído',
    table: { label: 'Criar sua primeira tabela', hint: 'Onde os dados do seu app ficam guardados.', action: 'Criar tabela' },
    protect: { label: 'Proteger os dados', hint: 'Decida quem pode ver e alterar cada tabela.', action: 'Revisar acesso' },
    user: { label: 'Criar um usuário', hint: 'Uma conta para testar o login do seu app.', action: 'Criar usuário' },
    connect: { label: 'Conectar seu app', hint: 'Este passo conclui quando alguém fizer login pelo seu app.', action: 'Ver como' },
  },
  attention: {
    exposed: '{table} está visível para qualquer pessoa',
    exposedHint: 'Qualquer pessoa com o endereço da API pode ler e alterar todas as linhas.',
    blocked: '{table} está bloqueada',
    blockedHint: 'A proteção está ativa, mas sem regras: ninguém acessa pela API.',
    protect: 'Proteger',
    addRule: 'Criar regra',
  },
  summary: {
    tables: { one: '{count} tabela', other: '{count} tabelas' },
    users: { one: '{count} usuário', other: '{count} usuários' },
    policies: { one: '{count} regra de acesso', other: '{count} regras de acesso' },
    functions: { one: '{count} função', other: '{count} funções' },
  },
  tables: {
    heading: 'Tabelas',
    openEditor: 'Abrir editor',
    empty: 'Nenhuma tabela ainda',
    emptyBefore: 'Crie com',
    emptyMiddle: 'ou no',
    table: 'Tabela',
    rows: 'Linhas',
    protection: 'Proteção',
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
  description: 'Your project is live.',
  steps: {
    title: 'Next steps',
    progress: '{done} of {total}',
    hide: 'Hide',
    done: 'Done',
    table: { label: 'Create your first table', hint: 'Where your app keeps its data.', action: 'Create table' },
    protect: { label: 'Protect your data', hint: 'Decide who can see and change each table.', action: 'Review access' },
    user: { label: 'Create a user', hint: 'An account to try your app’s sign-in.', action: 'Create user' },
    connect: { label: 'Connect your app', hint: 'This step completes when someone signs in through your app.', action: 'See how' },
  },
  attention: {
    exposed: '{table} is visible to anyone',
    exposedHint: 'Anyone with the API address can read and change every row.',
    blocked: '{table} is locked',
    blockedHint: 'Protection is on but has no rules: nobody gets in through the API.',
    protect: 'Protect',
    addRule: 'Add a rule',
  },
  summary: {
    tables: { one: '{count} table', other: '{count} tables' },
    users: { one: '{count} user', other: '{count} users' },
    policies: { one: '{count} access rule', other: '{count} access rules' },
    functions: { one: '{count} function', other: '{count} functions' },
  },
  tables: {
    heading: 'Tables',
    openEditor: 'Open editor',
    empty: 'No tables yet',
    emptyBefore: 'Create them with',
    emptyMiddle: 'or in the',
    table: 'Table',
    rows: 'Rows',
    protection: 'Protection',
    view: 'view',
  },
  notFound: {
    title: 'Page not found',
    description: 'This address does not exist in this panel.',
    back: 'Back to the overview',
  },
}

export default { 'pt-BR': ptBR, en }
