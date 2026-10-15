import type { Messages } from '../index.svelte'
const ptBR = {
  docker_namespace_exists:
    'Já existem containers, volumes ou redes Docker reservados para esse nome. Inspecione-os por SSH antes de criar o projeto.',
  docker_unavailable:
    'Não foi possível consultar o Docker da VPS. Resolva o runtime antes de criar o projeto.',
  storageCleanupFailure:
    'O resultado foi confirmado na VPS, mas o navegador não conseguiu limpar o identificador local. Ele será consultado novamente ao recarregar.',
  lastStage: 'Última etapa registrada',
  preserveWrites:
    'A restauração já chegou à verificação. A recuperação verifica o estado atual e preserva gravações posteriores; não reaplica o backup.',
  operation_failed:
    'A execução falhou. Consulte o diagnóstico privado da VPS antes de recuperar.',
  interrupted: 'A execução foi interrompida.',
  worker_interrupted:
    'O worker foi interrompido; revise a última etapa antes de recuperar.',
  history_full:
    'O histórico atingiu o limite. Arquive apenas operações concluídas por SSH.',
  title: 'Projetos da VPS',
  subtitle:
    'Crie e acompanhe backends isolados com operações confirmadas e histórico persistente.',
  projects: 'Projetos',
  updates: 'Atualizações',
  create: 'Novo projeto',
  name: 'Nome do projeto',
  domain: 'Domínio da API',
  profile: 'Perfil do PostgreSQL',
  preview: 'Validar e revisar',
  review: 'Revise antes de confirmar',
  isolation: 'Este projeto terá banco, usuários, chaves e arquivos próprios.',
  ram: 'RAM da VPS',
  disk: 'Espaço livre',
  count: 'Projetos na VPS',
  version: 'Versão aprovada pelo operador',
  image: 'Imagem aprovada por digest',
  imagePending:
    'A assinatura e a origem da imagem serão verificadas antes de criar o projeto.',
  expires:
    'Esta revisão expira em cinco minutos e será validada novamente antes de executar.',
  blocked:
    'A operação está bloqueada. Corrija os requisitos e faça uma nova revisão.',
  acknowledge: 'Conferi o projeto, o domínio e o impacto desta operação.',
  rescue: 'Tenho acesso SSH e compreendo como recuperar a VPS.',
  typed: 'Digite exatamente a confirmação abaixo',
  password: 'Confirme a senha do operador',
  approve: 'Confirmar e executar',
  cancel: 'Cancelar revisão',
  busy: 'Processando…',
  submitted:
    'Pedido registrado na VPS. A operação continua mesmo se você fechar esta página.',
  uncertain:
    'Não foi possível confirmar a resposta. Consulte o histórico ou confirme novamente o mesmo pedido; a chave será reutilizada.',
  retry: 'Consultar estado novamente',
  pending: 'Pedido aguardando confirmação do resultado',
  history: 'Operações e recuperação',
  accepted: 'Pedido registrado',
  validating: 'Validando requisitos',
  provisioning: 'Preparando banco e arquivos',
  starting: 'Iniciando serviços',
  backing_up: 'Capturando backup consistente',
  applying: 'Aplicando a operação',
  verifying: 'Verificando o resultado',
  completed: 'Concluído e verificado',
  failed: 'Não concluído; nenhuma alteração destrutiva foi iniciada',
  needs_attention: 'Atenção do operador necessária',
  resume: 'Revisar recuperação',
  recovery:
    'A recuperação pode concluir a operação originalmente aprovada. Ela preserva os outros projetos. Inspecione o histórico e mantenha acesso SSH.',
  backup: 'Criar backup',
  restore: 'Restaurar backup',
  remove: 'Remover projeto',
  logs: 'Consultar logs',
  logService: 'Serviço',
  app: 'Backend',
  database: 'PostgreSQL',
  logPrivacy:
    'São exibidos somente metadados estruturados. Mensagens livres e SQL ficam nos logs privados da VPS para proteger dados e credenciais.',
  noLogs: 'Nenhum evento estruturado disponível.',
  noProjects: 'Nenhum projeto cadastrado.',
  noHistory: 'Nenhuma operação registrada.',
  backupFile: 'Backup selecionado',
  backupSaved: 'Backup preservado',
  selectBackup: 'Selecione um backup',
  destructive:
    'Esta operação substitui ou remove dados. Ela exige backup verificado e afeta apenas o projeto selecionado.',
  removeImpact:
    'O projeto será retirado do domínio; banco e volumes serão removidos. O backup final e a configuração ficarão no arquivo privado da VPS.',
  restoreImpact:
    'Os dados e arquivos serão substituídos pelo par selecionado. O estado atual será salvo antes; o projeto ficará em manutenção.',
  backupImpact:
    'Será criado um par consistente de banco e arquivos. Verifique também sua política de cópias fora da VPS.',
  offline:
    'Controlador indisponível. A operação aceita continua na VPS; ações ficam desabilitadas até reconectar.',
  unknown: 'Estado indisponível',
  healthy: 'Saudável',
  unhealthy: 'Não saudável',
  running: 'Em execução',
  exited: 'Parado',
  missing: 'Não encontrado',
  refresh: 'Atualizar estado',
  planExpired: 'A revisão expirou. Valide novamente antes de confirmar.',
  error:
    'A operação foi bloqueada ou a sessão precisa ser renovada. Consulte o estado e revise o pedido.',
  result: 'Resultado confirmado',
  storageFailure:
    'O navegador não conseguiu preservar o identificador do pedido. A execução está bloqueada para evitar duplicações após recarregar.',
  project_limit: 'Limite de projetos do host atingido.',
  project_exists: 'O nome ou a pasta já existem.',
  domain_exists: 'Domínio já usado por outro projeto ou pelo controlador.',
  domain_forbidden: 'Domínio fora do escopo aprovado na VPS.',
  dns_mismatch:
    'O DNS não aponta exclusivamente para os endereços aprovados da VPS.',
  dns_unavailable: 'Não foi possível resolver o DNS.',
  insufficient_memory:
    'O perfil não cabe na divisão estimada de RAM. Escolha outro perfil ou amplie a VPS.',
  insufficient_disk: 'Espaço livre abaixo da reserva exigida.',
  host_busy: 'Outra operação precisa terminar ou ser recuperada.',
  docker_required: 'A criação de vários projetos exige o runtime Docker.',
  shared_panel_required:
    'A criação pelo painel exige o login compartilhado dos painéis de projeto.',
  project_layout_changed:
    'A identidade, configuração, armazenamento ou PITR exige revisão por SSH.',
  backup_unavailable:
    'O par de backup selecionado não está disponível ou não é seguro.',
  metadata: 'Metadados do evento',
  copy: 'Copiar',
  copied: 'Copiado',
  progress: 'Etapas da operação',
  readOnly:
    'A criação e as operações do host precisam ser autorizadas por SSH no cadastro do operador.',
  successCreate:
    'Projeto criado. Abra o painel para criar tabelas e políticas de acesso.',
  openPanel: 'Abrir painel do projeto',
  max: 'Limite aprovado',
  estimated:
    'Os perfis são estimativas de configuração, não garantias de capacidade.',
  remaining: 'Validade da revisão',
  seconds: 'segundos',
  select: 'Selecionar projeto'
}
const en: Messages<typeof ptBR> = {
  docker_namespace_exists:
    'Containers, volumes or networks already reserve this name in Docker. Inspect them over SSH before creating the project.',
  docker_unavailable:
    'The VPS Docker runtime could not be queried. Resolve the runtime before creating the project.',
  storageCleanupFailure:
    'The VPS confirmed the outcome, but the browser could not clear the local identifier. It will be reconciled again after reload.',
  lastStage: 'Last recorded stage',
  preserveWrites:
    'The restore already reached verification. Recovery verifies the current state and preserves later writes; it does not replay the backup.',
  operation_failed:
    'Execution failed. Inspect private VPS diagnostics before recovery.',
  interrupted: 'Execution was interrupted.',
  worker_interrupted:
    'The worker was interrupted; review the last stage before recovery.',
  history_full:
    'History limit reached. Archive only completed operations over SSH.',
  title: 'VPS projects',
  subtitle:
    'Create and monitor isolated backends with confirmed operations and persistent history.',
  projects: 'Projects',
  updates: 'Updates',
  create: 'New project',
  name: 'Project name',
  domain: 'API domain',
  profile: 'PostgreSQL profile',
  preview: 'Validate and review',
  review: 'Review before confirming',
  isolation: 'This project gets its own database, users, keys and files.',
  ram: 'VPS RAM',
  disk: 'Free space',
  count: 'VPS projects',
  version: 'Operator-approved version',
  image: 'Approved image digest',
  imagePending:
    'Image signature and provenance will be verified before project creation.',
  expires:
    'This review expires in five minutes and is checked again before execution.',
  blocked:
    'Operation blocked. Resolve the requirements and prepare a new review.',
  acknowledge: 'I reviewed the project, domain and impact of this operation.',
  rescue: 'I have SSH access and understand how to recover this VPS.',
  typed: 'Type the exact confirmation below',
  password: 'Confirm your operator password',
  approve: 'Confirm and execute',
  cancel: 'Cancel review',
  busy: 'Working…',
  submitted:
    'Request recorded on the VPS. The operation continues when you close this page.',
  uncertain:
    'The response could not be confirmed. Check history or approve the same request again; its key will be reused.',
  retry: 'Check status again',
  pending: 'Request awaiting confirmation of its outcome',
  history: 'Operations and recovery',
  accepted: 'Request recorded',
  validating: 'Validating requirements',
  provisioning: 'Preparing database and files',
  starting: 'Starting services',
  backing_up: 'Capturing a consistent backup',
  applying: 'Applying operation',
  verifying: 'Verifying outcome',
  completed: 'Completed and verified',
  failed: 'Not completed; no destructive changes were started',
  needs_attention: 'Operator attention required',
  resume: 'Review recovery',
  recovery:
    'Recovery may finish the originally approved operation. Other projects are preserved. Inspect history and retain SSH access.',
  backup: 'Create backup',
  restore: 'Restore backup',
  remove: 'Remove project',
  logs: 'View logs',
  logService: 'Service',
  app: 'Backend',
  database: 'PostgreSQL',
  logPrivacy:
    'Only structured metadata is displayed. Free-form messages and SQL remain in private VPS logs to protect data and credentials.',
  noLogs: 'No structured events available.',
  noProjects: 'No projects registered.',
  noHistory: 'No operations recorded.',
  backupFile: 'Selected backup',
  backupSaved: 'Preserved backup',
  selectBackup: 'Select a backup',
  destructive:
    'This operation replaces or removes data. It requires a verified backup and affects only the selected project.',
  removeImpact:
    'The project leaves its domain; database and volumes are removed. Its final backup and configuration remain in the private VPS archive.',
  restoreImpact:
    'Data and files are replaced with the selected pair. Current state is saved first; the project enters maintenance.',
  backupImpact:
    'A consistent database/file pair is created. Check your off-host copy policy too.',
  offline:
    'Controller unavailable. Accepted work continues on the VPS; actions remain disabled until reconnection.',
  unknown: 'Status unavailable',
  healthy: 'Healthy',
  unhealthy: 'Unhealthy',
  running: 'Running',
  exited: 'Stopped',
  missing: 'Not found',
  refresh: 'Refresh status',
  planExpired: 'Review expired. Validate it again before approval.',
  error:
    'Operation blocked or session needs renewal. Check status and review the request.',
  result: 'Outcome confirmed',
  storageFailure:
    'The browser could not preserve the request identifier. Execution is blocked to prevent duplicates after reload.',
  project_limit: 'Host project limit reached.',
  project_exists: 'The name or folder already exists.',
  domain_exists: 'Domain already used by another project or the controller.',
  domain_forbidden: 'Domain outside the approved VPS scope.',
  dns_mismatch: 'DNS does not resolve exclusively to approved VPS addresses.',
  dns_unavailable: 'DNS lookup failed.',
  insufficient_memory:
    'Profile exceeds the estimated RAM share. Select another profile or expand the VPS.',
  insufficient_disk: 'Free space is below the required reserve.',
  host_busy: 'Another operation must finish or be recovered.',
  docker_required: 'Multi-project provisioning requires Docker.',
  shared_panel_required: 'Panel creation requires shared project-panel login.',
  project_layout_changed:
    'Identity, configuration, storage or PITR requires SSH review.',
  backup_unavailable: 'The selected backup pair is unavailable or unsafe.',
  metadata: 'Event metadata',
  copy: 'Copy',
  copied: 'Copied',
  progress: 'Operation steps',
  readOnly:
    'Host creation and operations require explicit SSH authorization during operator enrollment.',
  successCreate:
    'Project created. Open its panel to create tables and access policies.',
  openPanel: 'Open project panel',
  max: 'Approved limit',
  estimated: 'Profiles estimate configuration, not guaranteed capacity.',
  remaining: 'Review validity',
  seconds: 'seconds',
  select: 'Select project'
}
export default { 'pt-BR': ptBR, en }
