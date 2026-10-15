# G05 — Registro técnico da execução

Status: em execução. O mínimo solicitado de oito horas efetivas e os endurances
de quatro horas por backend ainda não foram concluídos. Preparação, compilação,
esperas e verificações estáticas não contam como tempo de carga.

## Identidade e ambiente

- Branch: `test/g05-stress-verification`.
- Base do backend: `2efede8c27ee12119142170e8bd9742ff7cb900e`.
- Binário release compilado com `--locked`, Rust 1.99 e o perfil do repositório.
  SHA-256: `75c44c85a85db26195166aa1d2c6caf481e3db476801ec8131525ef62adf1be0`.
- Staging descartável neste ambiente cloud; nenhum destino de produção.
- Cinco CPUs visíveis, quota cgroup de quatro CPUs e 32 GiB de RAM.
- App, PostgreSQL e k6 em slots de CPU separados; S3 ocupa um quarto slot.
  Prometheus tem quota adicional explícita de 0,25 CPU e 512 MiB.
- App: 128 MiB; PostgreSQL: 384 MiB; gerador: 512 MiB; RustFS: 512 MiB.
  O consumo agregado e o throttling do host precisam acompanhar os resultados.
- O perfil atual reserva 0,5 CPU ao RustFS e 0,25 CPU ao coletor; com app,
  PostgreSQL e gerador a 1 CPU cada, restam 0,25 CPU da quota para observação
  e sistema. O preflight verifica a quota efetiva, não só CPUs visíveis.
- Banco em volume de disco próprio, sem tmpfs. Arquivos, volumes e contêineres
  recebem identidade exclusiva e descarte limitado ao respectivo run.
- Reservas mínimas: 10% de RAM disponível e 15% de bytes/inodes livres.
  Watchdog independente, heartbeat, prazo de seis horas por campanha e parada
  por perda de observação, OOM, serviço inesperadamente parado ou limite de disco.

## Correções e extensões em validação

| Item | Evidência anterior | Alteração |
|---|---|---|
| Timestamp não finito | Uma regressão reproduziu aceitação de `NaN` como cobertura válida. | Rejeitar relógio inválido/regressivo e conservar cobertura agregada em espaço constante. |
| Endurance | Executor aceitava no máximo uma hora; o descarte de amostras perdia o prefixo necessário à validação. | Suporte limitado a quatro horas contínuas e cobertura independente da janela de diagnóstico. |
| Memória do gerador | O `TrendSink` do k6 1.6.1 armazena cada valor para o resumo final. | Desativar esse coletor em sessões longas; coletar histogramas nativos com Prometheus descartável. A validação de erros continua no script. |
| Credenciais da fixture | Alguns comandos Docker recebiam valores em argumentos; JWTs sintéticos duravam duas horas. | Transportar segredos pelo ambiente; renovar credenciais de usuários por login/refresh; limitar a credencial administrativa de fixture à campanha. |
| Fidelidade das métricas | Capacidade HTTP era aplicada como taxa de iterações; latências incluíam rejeições. | Separar unidades, throughput útil, rejeições esperadas e latências de respostas bem-sucedidas. |
| Eventos | A janela em memória não comprovava o conjunto completo de eventos entregues. | Persistir identidades/hashes em SQLite e reconciliar com identidades de commits no PostgreSQL; registrar duplicatas separadamente. |
| Comparação | Estimadores e perfis novos poderiam não participar da equivalência. | Rejeitar comparações com protocolo, SLOs, manutenção ou recursos diferentes. |
| Interrupção | A preparação não tinha watchdog independente e havia caminhos sem resultado persistido. | Monitorar também a preparação, persistir falhas, registrar tráfego e só aprovar após confirmar teardown. |

O perfil prolongado declara manutenção explícita do histórico terminal de
webhooks em 5.000 registros, pela API administrativa documentada, preservando
identidades em uma tabela de auditoria da fixture. O limite de 100.000 registros
do produto permanece intacto. Sem manutenção, ele pode limitar a duração de
tráfego contínuo; esse comportamento exige cenário próprio, sem interpretar
backpressure documentado como perda silenciosa de eventos.

Dois pequenos grupos de sessões/tokens expirados e eventos concluídos são
envelhecidos somente na fixture, durante o endurance, para observar os pruners
reais. Limpeza manual do histórico aguarda a remoção desses eventos pelo pruner.

## Checkpoints iniciais

- Build release da base concluído e cópia imutável do executável preservada.
- Smoke em disco com reinício e 60 segundos de soak: passou.
- Primeiro smoke com Prometheus: falhou por permissão de leitura da configuração.
  O arquivo não contém segredos; permissões de leitura e UID foram corrigidos.
  A repetição em disco passou com a nova coleta de histogramas.
- Smoke S3 real com 90 segundos de soak: passou; 4.719 eventos únicos observados,
  zero ausentes/extras e bytes verificados em 33 objetos.
- Regressões Python relacionadas: 41 passaram na última verificação registrada.
  O registro final atualizará a contagem e o SHA correspondente.
- Preflight com SLOs ativos: reprovou o login, p95 de 995 ms com alvo de
  750 ms, em carga estimada de 572 iterações/s. O critério foi preservado;
  a calibração passou a qualificar taxas por chegadas controladas, SLOs e
  entrega, em vez de assumir capacidade sustentável a partir de uma amostra curta.
- Watchdog com perda simulada do processo controlador: passou. O recurso com
  identidade correspondente foi removido; um recurso de teste com outro label
  sobreviveu. A fixture PostgreSQL anterior permaneceu com a mesma identidade.
- `disk-pass-1-001` foi interrompida pelo agente antes do endurance: os arquivos
  estavam no `/tmp` do host, que é tmpfs. Foram preservados 735 segundos de
  intervalos concluídos, sem certificação de disco real; teardown confirmado.
  Os smokes anteriores do adaptador local também usaram essa topologia e
  demonstram funcionalidade, não desempenho de disco físico. A fixture foi
  movida para o filesystem de disco; o gate rejeita tmpfs/ramfs e o comparador
  considera o tipo de filesystem.

Esses smokes não certificam capacidade máxima, oito horas de carga ou conclusão
do G05. As evidências brutas e falhas ficam em `target/stress-g05/`.

## Situação dos critérios

Os doze critérios do G05 permanecem abertos até a consolidação de campanhas,
regressões, duas passagens e revisão do artefato final. Recursos indisponíveis
serão registrados como bloqueados, sem substituição silenciosa por mocks.
Não haverá deploy de produção ou merge automático.

## Checkpoint de correção do executor

- `disk-pass-1-002` foi reprovada pelo guard de observação; o diagnóstico
  `stats-race-diagnostic-001` reproduziu `EOF` ao encerrar o gerador entre fases.
  A coleta agora usa o ID imutável e a geração do processo, sem ignorar erros
  de um gerador ainda vivo. A repetição `stats-race-verification-001` passou
  por 210 segundos de carga concluída, reinício e teardown.
- `disk-pass-1-003` foi interrompida antes do endurance para ampliar a folga
  de disco. Foram conservados 195 segundos de carga e o teardown confirmado.
  O preflight agora exige espaço para o crescimento declarado acima da reserva
  de 15%. Somente caches de compilação foram movidos para `/tmp`; dados de
  bancos/arquivos permanecem no disco. Os caches e seus caminhos foram preservados.
- O primeiro smoke do runner de projetos independentes falhou na consulta
  escalar do Prometheus e na verificação do filtro de teardown. Ambos foram
  corrigidos. A repetição `fleet-1-smoke-002` passou por três intervalos de
  30 segundos, com métricas individuais e cleanup confirmado. É um smoke,
  sem certificação de etapas de cinco minutos ou de capacidade de dez projetos.
- `disk-pass-1-004` está em andamento com fonte congelada e o mesmo executável
  imutável. O status final depende das quatro horas contínuas, invariantes,
  retenção, recursos e cleanup; iniciar a campanha não aprova o endurance.


## Primeiro endurance em execução

A campanha `disk-pass-1-004` concluiu 4.395 segundos de carga antes do endurance.
Cada qualificação abaixo durou cinco minutos, em iterações/s e com histogramas
nativos no perfil local declarado.

| Taxa | Qualificação | p95 de login |
|---:|---|---:|
| 183 | passou | 42 ms |
| 367 | passou | 55 ms |
| 551 | passou | 142 ms |
| 735 | falhou: SLOs e chegadas descartadas | 2.349 ms |
| 642 | falhou: SLO de login | 867 ms |
| 596 | falhou: SLOs de login | 1.493 ms |
| 573 | passou | 320 ms |

O comportamento não foi monotônico entre amostras; as falhas foram preservadas.
A campanha repetiu capacidade, saturação, cinco ciclos de pico/recuperação e
reinício antes de iniciar 14.400 segundos contínuos a 458 iterações/s. O resultado
ainda está pendente. Preparação e espera não entram nos 4.395 segundos concluídos.

Esse snapshot usa 10.000 registros de listagem sintéticos da fixture anterior.
A integridade cobre os objetos efetivamente publicados pela API. O candidato
seguinte cria 10.000 objetos vazios reais e verifica inventário, tamanho e hash.
Perfis diferentes não serão apresentados como uma comparação equivalente.

## Correções adicionais em validação

- Docker verifica ownership e usa IDs imutáveis. Volumes estrangeiros são
  recusados antes de montar; S3 recebe volume nomeado e rotulado.
- A parada crítica valida PID, starttime, UID e cgroup do gerador, com sinalização
  independente da API Docker. Recibos concorrentes usam temporários distintos.
- Contadores cgroup detectam OOM de filho PostgreSQL com postmaster ainda vivo.
  Neste ambiente, parte dos cgroups está fora do mount do controlador; a leitura
  usa o namespace do contêiner quando necessário.
- A comparação usa trabalho bem-sucedido e exige campanha final aprovada e
  cleanup. Rejeições HTTP não podem mascarar uma regressão; boas amostras
  iniciais não aprovam uma campanha que falhou depois.
- Há perfis de um milhão de linhas, transferências até 1 MiB, inventário físico,
  duas coortes de GC e sessão contínua renovada na expiração. A validação
  integrada e as repetições no candidato final permanecem pendentes.
- O runner de falhas tem ledger de transferências, saldos e contador atômico;
  inclui desconexões, deadlock, reinício, OOM marcado, quotas de bytes/inodes,
  snapshot sob escritores e restauração independente. Ainda não foi executado.
- O workflow publica somente bundle estruturado com remoção de segredos;
  logs brutos, fixtures e dumps permanecem privados. Exportar não certifica G05.

O inventário `results/stress/coverage-inventory.json` distingue cenários não
executados e destinos indisponíveis. Os scripts legados que exigem cgroup do
host privilegiado ou bypass de TLS não foram executados. Não há VM descartável
com systemd, DNS público, destino off-host ou WAL archive/PITR configurado
para os respectivos gates neste ambiente.
