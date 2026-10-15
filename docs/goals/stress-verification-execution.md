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
- Regressões Python: a descoberta completa de `scripts/review_tests` passou com
  98 testes em 2,577 s no commit `855e8f7`. As 11 regressões de ownership/portas
  passaram após a correção seguinte. Os 18 checks de CI também passaram em `855e8f7`.
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
- `disk-pass-1-004` usou fonte congelada e o executável imutável. A campanha
  foi reprovada e removida com cleanup confirmado; o diagnóstico está abaixo.


## Primeira tentativa de endurance

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
reinício antes de iniciar uma tentativa de 14.400 segundos contínuos a 458
iterações/s. A tentativa foi interrompida após 10.679,235 segundos por uma falha
inesperada. Não há aprovação de quatro horas. Preparação e espera não entram nos
4.395 segundos de intervalos concluídos; a carga interrompida é registrada à parte.

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

## Falhas observadas às 07:34–07:43 UTC

- O endurance foi interrompido por um `PATCH` com HTTP 504. O PostgreSQL registrou
  statement timeout de três segundos ao atualizar `webhook_queue_usage`, além
  de espera na linha de `auth.users` no mesmo instante. A causa raiz ainda está
  em investigação; contenção e I/O são hipóteses, não conclusões.
- As duas coortes de auth e webhooks foram removidas pelos pruners reais.
  No fim observado, RSS era 48.728 KiB, oito threads; descritores caíram de 152
  para 24 quando o gerador parou. A fila drenou a zero e não havia transação idle
  no último sample. A reconciliação final não executou, portanto não há aprovação
  integral de integridade ou recursos para essa tentativa.
- O smoke `fault-real-s3-smoke-001` falhou na recuperação do banco, antes do S3.
  Uma reprodução isolada comprovou mudança de porta Docker após stop/start:
  `32800` → `32801`. O executor agora usa porta explícita em loopback e confere
  sua identidade. A repetição isolada preservou a porta; a campanha afetada será
  repetida. Ambos os runs falhos tiveram teardown confirmado.
- O novo diagnóstico guarda somente em staging estatísticas e logs privados,
  com snapshot sintético opcional limitado a 64 MiB e 30 segundos. A coleta é
  pulada em paradas críticas. Seu resultado não transforma falha em aprovação.

## Checkpoint de 09:25 UTC

`s3-pass-1-001` começou às 09:22 UTC com snapshot congelado do commit
`6d93f8b`, 10.000 objetos físicos, 100.000 linhas, WAL de 256 MiB e endurance
a 70% da taxa que passar na qualificação de cinco minutos. Os SLOs, entrega
mínima de 99% e limite de descarte de 1% foram preservados. Essa é uma nova
medição: alterar o perfil não comprova a causa ou a resolução do 504 anterior.
Nenhum endurance de quatro horas está aprovado neste checkpoint.

- `filesystem-diagnostic-002`: passou, com 130 segundos de carga concluída,
  27 amostras, 910 escritas confirmadas/conciliadas e zero ausentes/corrompidas.
  Esgotamento de bytes retornou `507 storage_full`; inodes retornou `500 internal`.
  Em ambos os casos, houve 18 commits durante a falha e novos uploads íntegros
  após recuperação. O arquivo anterior permaneceu acessível. Cleanup confirmado.
- A causa da falha de fixture foi reproduzida antes de aplicar pressão:
  `docker cp` deixou o tmpfs visto pela aplicação vazio, resultando em 404.
  O transporte limitado por tar/`docker exec` popula o mount usado pelo processo
  e verifica os bytes antes de injetar a falha.
- `provider-diagnostic-001`: passou, com 130 segundos de carga, RustFS real,
  936 escritas confirmadas/conciliadas e zero ausentes/corrompidas. Reinício e
  pausa tiveram RTO de 0,490 s e 0,108 s após restaurar o provedor; leitura,
  nova escrita e rejeição de credencial inválida foram verificadas. As respostas
  500/503 e conexões indisponíveis ficaram limitadas à janela de falha prevista.
- `s3-extended-smoke-001` foi reprovado pelo sampler: `du` encontrou versões
  removidas durante sobrescritas. A coleta agora conta somente `ENOENT` de
  filhos de um root próprio com total válido. Outros erros continuam fatais.
  `s3-extended-smoke-002` atravessou 47 dessas corridas, preservando as medidas,
  mas reprovou a entrega na quinta recuperação: 6.035 iterações e 166 descartes
  para 6.200 chegadas previstas. A reprovação foi preservada.
- Essa recuperação realizou 409 logins onde a mistura declarava aproximadamente
  302: novos VUs inicializavam sessões dentro da carga. O candidato prepara
  sessões reais antes do intervalo e mantém 5% de hashing por uma sequência
  global de chegadas. O bootstrap possui métricas separadas e não conta como
  duração efetiva. O controle nativo de composição confirmou a mistura.
- O oráculo anterior aceitava HTTP 200 com bytes incorretos. O controle nativo
  reproduziu a falsa aprovação; o candidato aprovou bytes corretos e abortou
  para corrupção e truncamento. Esses controles usam um servidor de teste
  limitado para validar o benchmark e não certificam o backend ou S3.
- A descoberta Python completa passou com 117 testes. Os testes de sockets
  verificam descarte da resposta, ordem temporal, causa de erro e ausência de
  crescimento de descritores/tarefas. A nova campanha de perda de resposta
  após COMMIT ainda precisa rodar contra o backend real.

Os dois diagnósticos focados não aprovam a matriz inteira de falhas. As tentativas
`fault-real-s3-smoke-001` a `005`, o primeiro diagnóstico de filesystem e os dois
smokes S3 reprovados permanecem nas evidências, com respectivos cleanups.
O mínimo de oito horas efetivas e os doze critérios continuam abertos.

## Repetição de capacidade reprovada às 10:31 UTC

`s3-pass-1-001` terminou reprovado antes do endurance, com 4.035 segundos de
carga concluída, 601 amostras e cleanup confirmado. A qualificação a 596
iterações/s passou uma vez, mas a repetição reprovou p95 de leitura (226 ms),
RLS (229 ms), refresh (246 ms) e storage (583 ms). Os limites continuam
200/200/200/500 ms, respectivamente. Não houve aprovação por média entre
amostras boas e ruins.

A mediana de CPU da aplicação subiu de 91,1% na primeira amostra a 96,7% de
um núcleo na repetição. O gerador ficou em 30,4% e 34,2%; a limitação observada
não foi sua CPU. A etapa a 447 iterações/s passou com p95 de leitura 14,8 ms,
login 79,4 ms e storage 58,0 ms. A memória anônima do RustFS aproximou-se de
455 MiB no limite de 512 MiB, sem OOM observado; uma hora não distingue cache,
retenção de alocador e crescimento de longo prazo.

A próxima medição declara previamente teto de qualificação de 400 iterações/s
e 1.024 MiB para RustFS, mantendo os demais budgets, a duração de cinco minutos,
as rejeições, os SLOs e a exigência de entrega. O teto é um limite conservador
para obter um ponto operacional repetível, não uma alegação de capacidade
máxima. Os overloads continuam acima da taxa que efetivamente qualificar.
O endurance só contará depois de iniciado e aprovado integralmente.

## Extensão de isolamento com webhooks

A revisão encontrou uma lacuna no executor de múltiplos projetos: a etapa de
escrita não configurava endpoints, portanto não exercitava workers, assinaturas
ou filas de webhooks. O perfil opcional `--webhooks` agora configura um receiver
local próprio por projeto antes do baseline. A escrita ruidosa também produz
entregas reais, com ledger persistente de IDs gravados no PostgreSQL e recebidos.
Ausência de tráfego positivo, assinatura inválida, identidade ausente/estranha,
fila sem drenar em 30 segundos ou evidência acima de 128 MiB reprova a campanha.
Duplicatas preservam sua identidade e são contadas separadamente. O comparador
recusa perfis com configuração de webhooks diferente.

O teste de oráculos passou com oito casos; a descoberta Python completa passou
com 126 testes em 5,448 segundos. Esses testes validam a extensão e não aprovam
o cenário real de 1/5/10 projetos, que ainda precisa executar. A campanha S3
ativa permanece no snapshot congelado `6d77010`; a extensão do host não muda
o binário nem o JavaScript medido por ela. Nenhum endurance está aprovado neste
checkpoint.

## Início do endurance S3 às 11:40 UTC e preparação de ingress

`s3-pass-1-002` concluiu 3.495 segundos de carga antes do endurance. A repetição
de capacidade a 100/200/300/400 iterações/s passou, sem descartes nem rejeições.
Na amostra a 400, p95 de leitura/login/storage foi 11,9/68,8/44,0 ms. Os pontos
de sobrecarga a 600 e 800 mostraram descartes e maior latência; não são taxas
nominais aprovadas. Os cinco ciclos de pico/recuperação foram concluídos antes
do início do intervalo contínuo de 14.400 segundos, a 280 iterações/s.
O resultado do endurance e sua conciliação final ainda estão pendentes.

Foi preparado um perfil opcional `--caddy` para o executor de host. Ele mantém
o Caddy real em container próprio, com quotas de 0,25 CPU/128 MiB, orçamento de
64 MiB em disco, endpoints HTTPS locais e uma CA interna nova. O cliente verifica
o certificado; um controle adicional exige rejeição sem essa CA. A imagem é
executada pelo ID imutável observado. A exceção de compressão para storage e os
headers seguem o template de deployment. A preparação passou na descoberta
Python com 131 testes em 5,940 segundos. Nenhuma campanha nativa com esse perfil
foi executada neste checkpoint. DNS público, ACME, controller, falhas de proxy
e limites de clientes lentos continuam sem aprovação por esse perfil.

## Extensão do teste de admissão no histórico completo

Foi adicionado um teste Rust com PostgreSQL real para 100.000 registros de
histórico. Ele captura eventos reais de 100.000 escritas de fixture; 99.999 são
cancelados explicitamente durante a preparação, e duas entregas positivas usam
um receiver HTTP local com validação das assinaturas. O teste exige que 16 novas
escritas concorrentes sejam rejeitadas com `503 unavailable`/SQLSTATE `53300`,
sem gravar as linhas de negócio, e que replay, leitura de status, RLS e rejeição
de JWT inválido mantenham os contratos. Somente depois de provar a admissão cheia
ele executa a limpeza explícita e verifica nova escrita/entrega, preservando as
100.000 linhas de negócio anteriores. O contador de uso não é forjado.

A formatação do novo teste foi verificada. Sua execução Rust local e no CI ainda
está pendente neste registro; preparar o caso não equivale a aprová-lo. O
endurance S3 permanece no artefato congelado, sem compilação local concorrente.

## Fronteiras de journal e inventário

Os 18 checks do commit `9f9a0eb` passaram, incluindo `cargo test --all`, o ciclo
de host com Docker e os testes explícitos de S3 real. Isso fornece evidência
funcional do teste de histórico cheio nesse commit; não substitui o endurance
do binário release ou a qualificação de múltiplos projetos.

Foram preparados dois casos adicionais de fronteira. O journal cria 255 recibos
válidos, preserva os arquivos byte a byte e verifica 16 leitores/tentativas
concorrentes: retries mantêm a identidade e novas operações recebem o bloqueio
de capacidade, inclusive após reabrir o journal. Outro operador não pode usar
a chave de um recibo existente. O teste de lifecycle com root isolado também
preenche 256 identidades retiradas em diretório próprio, exige rejeição da 257ª
sem exclusão ou reescrita das anteriores, e confirma que atualizar uma identidade
existente não autoriza um runtime ausente. O inventário de fixture é separado
dos projetos usados no lifecycle. A formatação passou; a execução desses dois
novos casos ainda está pendente neste checkpoint.

## Reprovação do oráculo de contador esparso às 13:05 UTC

Os 18 checks de `c38ce90` passaram, incluindo as fronteiras de journal e inventário.
Uma inspeção da janela móvel de cinco minutos do S3 também encontrou 280,01
operações úteis/s, zero descartes e todos os SLOs dentro dos limites. Porém, ela
reproduziu um defeito no encerramento do benchmark: a consulta instantânea do
contador de renovações retornou evidência ausente, enquanto a série temporal
preservava cinco renovações e cinco refreshes positivos do mesmo canário, com
5.081 iterações. O k6 só emite esse contador quando ocorre o evento; ele pode
sumir da consulta instantânea após o lookback padrão de cinco minutos do Prometheus.

A correção lê a última amostra regular dentro da hora retida e recusa resets,
evidência ausente ou não finita. Ela retornou as cinco renovações contra o
Prometheus real da mesma tentativa antes da interrupção. O mínimo de três
renovações e a exigência de iterações contínuas foram preservados. A reprodução
antes/depois foi registrada em
[sparse-counter-before-after.json](results/stress/sparse-counter-before-after.json).
A descoberta Python passou com 136 testes em 5,785 segundos, incluindo casos
para ausência de amostra, reset, NaN e seletor/janela inválidos.

`s3-pass-1-002` foi interrompido de forma controlada para corrigir e repetir o
intervalo afetado com fixture nova. O resultado ficou `failed`, com 3.495 segundos
de carga concluída e cleanup confirmado. O endurance parcial, de aproximadamente
85 minutos de processo gerador, não conta como quatro horas aprovadas; tampouco
houve conciliação final de toda a campanha. O primeiro ciclo de auth/eventos/GC
físico foi observado completo e a continuidade de processos foi verificada,
mas essas evidências parciais não aprovam o requisito de endurance.
