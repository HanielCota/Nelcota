// API page: endpoints, roles, service_role token and code examples.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'API',
  navigation: 'Seções da documentação da API',
  description: 'Como o seu app conversa com este projeto.',
  openapi: 'Documentação OpenAPI',
  address: 'Endereço do projeto',
  copyAddress: 'Copiar endereço',
  endpoints: {
    rest: 'tabelas, views e funções (rpc) do schema exposto',
    auth: 'cadastro, login, sessão e usuário atual',
    jwks: 'chave pública para validar os JWTs',
  },
  caller: 'Quem faz a chamada',
  callerHint:
    'Não existe chave anon: quem não manda token já é anon. O acesso de cada role é definido por tabela, nos GRANTs (aba Estrutura) e nas políticas.',
  roles: {
    anon: {
      title: 'Visitante',
      text: 'Request sem header Authorization. Vê o que os GRANTs e as políticas liberam para anon.',
    },
    authenticated: {
      title: 'Usuário logado',
      text: 'Authorization: Bearer com o access_token do login. auth.uid() é o id dele nas políticas.',
    },
    service_role: {
      title: 'Seu backend',
      text: 'Bearer com o token service_role. Ignora o RLS (mas precisa de GRANT na tabela): só em código que roda no servidor.',
    },
  },
  neverInBrowser: 'Nunca use no navegador.',
  examples: 'Exemplos',
  topic: 'Assunto',
  topics: {
    tables: 'Tabelas',
    auth: 'Autenticação',
  },
  table: 'Tabela',
  language: 'Linguagem',
  noTables: 'Crie uma tabela para ver exemplos com as colunas dela.',
  types: 'Tipos TypeScript das tabelas:',
  snippets: {
    list: {
      label: 'Listar linhas',
      description: 'As 20 primeiras (order=coluna.desc ordena). Sem token, vale o que a role anon pode ver.',
    },
    filter: {
      label: 'Filtrar',
      description: 'Operadores: eq, neq, gt, gte, lt, lte, ilike, in, is (ex.: {column}=ilike.*abc*).',
    },
    insert: {
      label: 'Inserir',
      description: 'Colunas ausentes recebem o DEFAULT. Prefer: return=representation devolve a linha criada.',
    },
    update: {
      label: 'Atualizar',
      description: 'PATCH exige filtro: sem ele a API recusa, para não alterar a tabela inteira.',
    },
    delete: {
      label: 'Apagar',
      description: 'DELETE também exige filtro.',
    },
    signup: {
      label: 'Criar conta',
      description: 'Devolve a sessão (access_token e refresh_token) já logada.',
    },
    login: {
      label: 'Entrar',
      description: 'O access_token vai no header Authorization das chamadas à API (role authenticated).',
    },
    refresh: {
      label: 'Renovar a sessão',
      description: 'O access_token expira em minutos; troque o refresh_token por um par novo.',
    },
  },
  code: {
    label: 'Código',
  },
  token: {
    title: 'Token',
    introBefore: 'Para o seu backend.',
    introWarning: 'Ignora todo o RLS',
    introAfter:
      ': nunca coloque no frontend, num app mobile ou num repositório. Cada token vale até expirar e ainda depende de GRANT em cada tabela (as criadas pelo painel já dão acesso total ao service_role).',
    copy: 'Copiar token',
    copyNow: 'Copie agora: ele não será mostrado de novo. Vale até {date}.',
    validity: 'Validade',
    days: { one: '{count} dia', other: '{count} dias' },
    oneYear: '1 ano',
    generating: 'Gerando…',
    generateAnother: 'Gerar outro',
    generate: 'Gerar token',
    orOnServer: 'ou no servidor:',
  },
}

const en: Messages<typeof ptBR> = {
  title: 'API',
  navigation: 'API documentation sections',
  description: 'How your app talks to this project.',
  openapi: 'OpenAPI documentation',
  address: 'Project address',
  copyAddress: 'Copy address',
  endpoints: {
    rest: 'tables, views and functions (rpc) of the exposed schema',
    auth: 'sign-up, login, session and current user',
    jwks: 'public key to validate the JWTs',
  },
  caller: 'Who makes the call',
  callerHint:
    'There is no anon key: whoever sends no token is already anon. Each role’s access is defined per table, in the GRANTs (Structure tab) and in the policies.',
  roles: {
    anon: {
      title: 'Visitor',
      text: 'Request without an Authorization header. Sees what the GRANTs and policies allow for anon.',
    },
    authenticated: {
      title: 'Signed-in user',
      text: 'Authorization: Bearer with the access_token from login. auth.uid() is their id in policies.',
    },
    service_role: {
      title: 'Your backend',
      text: 'Bearer with the service_role token. Bypasses RLS (but still needs a GRANT on the table): only in code that runs on the server.',
    },
  },
  neverInBrowser: 'Never use it in the browser.',
  examples: 'Examples',
  topic: 'Topic',
  topics: {
    tables: 'Tables',
    auth: 'Authentication',
  },
  table: 'Table',
  language: 'Language',
  noTables: 'Create a table to see examples with its columns.',
  types: 'TypeScript types for the tables:',
  snippets: {
    list: {
      label: 'List rows',
      description: 'The first 20 (order=column.desc sorts). Without a token, you get what the anon role can see.',
    },
    filter: {
      label: 'Filter',
      description: 'Operators: eq, neq, gt, gte, lt, lte, ilike, in, is (e.g. {column}=ilike.*abc*).',
    },
    insert: {
      label: 'Insert',
      description: 'Missing columns get their DEFAULT. Prefer: return=representation returns the created row.',
    },
    update: {
      label: 'Update',
      description: 'PATCH requires a filter: without one the API refuses, so the whole table is not changed.',
    },
    delete: {
      label: 'Delete',
      description: 'DELETE also requires a filter.',
    },
    signup: {
      label: 'Sign up',
      description: 'Returns the session (access_token and refresh_token), already signed in.',
    },
    login: {
      label: 'Sign in',
      description: 'The access_token goes in the Authorization header of API calls (authenticated role).',
    },
    refresh: {
      label: 'Refresh the session',
      description: 'The access_token expires in minutes; trade the refresh_token for a new pair.',
    },
  },
  code: {
    label: 'Code',
  },
  token: {
    title: 'Token',
    introBefore: 'For your backend.',
    introWarning: 'It bypasses all RLS',
    introAfter:
      ': never put it in a frontend, a mobile app or a repository. Each token lasts until it expires and still depends on a GRANT on each table (tables created by the panel already give service_role full access).',
    copy: 'Copy token',
    copyNow: 'Copy it now: it will not be shown again. Valid until {date}.',
    validity: 'Validity',
    days: { one: '{count} day', other: '{count} days' },
    oneYear: '1 year',
    generating: 'Generating…',
    generateAnother: 'Generate another',
    generate: 'Generate token',
    orOnServer: 'or on the server:',
  },
}

export default { 'pt-BR': ptBR, en }
