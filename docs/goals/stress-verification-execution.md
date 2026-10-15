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
- Regressões Python relacionadas: 27 passaram na última verificação registrada.
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
