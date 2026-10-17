# G05 — Registro técnico da execução

Status: em execução. O checkpoint conservador 004 registra 17h34m14s de
intervalos de carga concluídos, acima do mínimo de oito horas; campanhas
posteriores ainda serão somadas. As primeiras execuções completaram quatro
horas contínuas em disco e quatro em S3 real local, com completion histórico
`passed`, integridade e continuidade registradas. A auditoria posterior
confirmou que seus totais finais de descartes não foram medidos corretamente;
elas não certificam o protocolo de contadores corrigido e exigem substituição.
As passagens independentes, a matriz completa e a aprovação do G05 continuam
pendentes. A errata de contadores abaixo prevalece sobre os checkpoints antigos.
Preparação, compilação, esperas, trechos interrompidos e verificações estáticas
não contam nesse total. Falhas de fairness/drenagem continuam reprovadas.

## Identidade e ambiente

Os limites abaixo descrevem o perfil inicial. Os manifestos de cada execução
e os checkpoints posteriores registram alterações declaradas antes da carga,
incluindo o perfil final de S3 com 1.024 MiB para o provider; não se comparam
perfis diferentes como equivalentes.

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

## Nova tentativa contínua às 14:14 UTC

`s3-pass-1-003` usa o snapshot congelado `1d53a67`, o mesmo binário imutável e
os mesmos budgets de WAL, memória, tráfego e SLOs. Ela repetiu a qualificação
inicial, as quatro etapas de capacidade de cinco minutos, overloads, concorrência,
cinco picos/recuperações e reinício antes do endurance, concluindo 3.495 segundos
de carga. O novo intervalo contínuo de 14.400 segundos começou às 14:14 UTC,
a 280 iterações/s. O encerramento e sua conciliação ainda estão pendentes.

Os 18 checks de `1d53a67` passaram. O inventário agora registra 14 subconjuntos
funcionais verificados no CI, com os links dos jobs e seus limites explícitos.
Nenhuma linha da matriz de stress foi promovida apenas por esses subconjuntos.
O resumo está em
[ci-functional-subsets.json](results/stress/ci-functional-subsets.json). As
contagens globais de testes Rust não foram extraídas dos logs; o relatório não
inventa esses números. O mínimo de oito horas e os dois passes completos
continuam sem aprovação neste checkpoint.


### 2026-10-15 14:40 UTC — interrupção preventiva e validação semântica

A execução `s3-pass-1-003` foi interrompida às 14:40:45 UTC, após cerca de
26 minutos da etapa contínua iniciada às 14:14:39. Os 3.495 segundos das
etapas anteriores foram concluídos e 803 observações foram preservadas; a
limpeza foi confirmada. Essa tentativa não concluiu endurance nem a
reconciliação final. O novo contador de renovação foi observado em operação,
mas o requisito final de três renovações ainda não havia sido atingido.

A revisão independente identificou respostas aceitas sem conferir integralmente
os dados retornados: leitura, painel, listagem, recibo de upload, identidade de
login, validade numérica da sessão e representação da escrita. O painel usava
`limit=20`, embora seu contrato seja `size=20`; por isso consultava o tamanho
padrão de 50 linhas. As novas verificações validam status específicos,
quantidade, filtro, ordenação, identidade, paginação, campos da fixture e
recibos com tamanho e SHA-256. Escritas usam `Prefer: return=representation`.

O controle nativo com k6 demonstrou onze falsos positivos no executor anterior
e a rejeição dos doze casos semânticos negativos no candidato; o caso de RLS
vazio já era rejeitado. Controles válidos, bytes corrompidos/truncados e a
distribuição do tráfego também mantiveram seus resultados esperados: 28/28
controles, limpeza confirmada. Os 136 testes Python passaram em 5,626 s e os
presets nativos base e performance/imports do Oxlint registraram zero achados.
A [evidência estruturada](results/stress/semantic-oracle-before-after.json)
certifica apenas os oráculos controlados. A validação contra o backend real
precede a retomada das campanhas longas. Nenhum endurance de quatro horas
está aprovado nesta atualização.


### 2026-10-15 15:20 UTC — validações reais e cancelamento seguro

Os testes `semantic-disk-smoke-001` e `semantic-s3-smoke-001` passaram com
150 segundos efetivos cada, respectivamente 39 e 36 amostras, reconciliação
final e limpeza. Cada inventário físico tinha 10.017 objetos: as 10.000
entradas vazias da fixture e 17 objetos verificados por bytes. Foram
reconciliados 1.553 eventos no disco e 1.549 no S3, sem identidades ausentes,
extras ou assinaturas inválidas. Os patamares finais não tinham transações
ociosas nem locks em espera. São diagnósticos curtos, com calibração de cinco
segundos, sem aprovação de capacidade ou endurance.
[Evidência](results/stress/semantic-real-backend-smokes.json).

A revisão encontrou ainda uma janela entre conferir a identidade do processo
controlador e sinalizar seu PID numérico. O watchdog agora abre um `pidfd`,
confere início/UID e envia SIGTERM pelo descritor, preservando a identidade
do processo. Duas regressões usam processos reais e proíbem o caminho com
`os.kill`; 138 testes Python passaram em 5,653 s. A aceitação nativa
`native-guards-002` provocou uma lacuna de amostragem de 31 segundos: o
watchdog encerrou um gerador real por pidfd/SIGKILL e seu controlador por
pidfd/SIGTERM, rejeitou quatro operações sobre recursos de outra fixture e
preservou seu container/volume até a limpeza explicitamente pertencente a
essa fixture. Limpeza confirmada em ambas.
[Evidência](results/stress/native-guards-pidfd.json).


### 2026-10-15 15:22 UTC — Caddy real, reload e dez projetos

As duas primeiras preparações de Caddy falharam antes do tráfego e foram
limpas. O segundo diagnóstico preservou `exec /usr/bin/caddy: operation not
permitted`; a leitura da imagem confirmou `cap_net_bind_service=ep`. Com
capabilities removidas do bounding set, esse arquivo não podia ser executado.
A fixture agora copia exatamente os bytes da imagem sem atributos de
capability, confere o SHA-256, monta o executável somente para leitura e
preserva usuário sem privilégios, `cap-drop ALL` e `no-new-privileges`. O
orçamento próprio do proxy passa a 128 MiB de disco, incluindo essa cópia,
sem alterar quotas de CPU/RAM nem SLOs.

`fleet-caddy-1-smoke-003` passou: 120 segundos de tráfego, 27 amostras, HTTPS
com CA verificada e controle negativo da CA não confiável, 24 eventos assinados
reconciliados e limpeza. Durante tráfego nominal, uma configuração válida
com listener adicional sobre uma porta ocupada pela aplicação foi realmente
rejeitada pelo admin de Caddy. A configuração original foi reaplicada,
consultas reais e JWT inválido/401 foram conferidos, o mesmo processo
permaneceu ativo e todos os gates nominais da etapa passaram. Recuperação
medida após reaplicação: 0,108 s; procedimento inteiro: 0,420 s. Isso não
certifica DNS/ACME público nem o controlador de produção.
[Evidência](results/stress/caddy-before-after.json).

A preparação inicial de dez projetos foi bloqueada por headroom insuficiente,
antes de criar recursos. Para preservar a mesma carga e os mesmos limites,
1.613.082.512 bytes de caches `.rlib` recentes foram transferidos, com hashes
conferidos e links preservados, para RAM disponível. O espaço livre real em
`/workspace` aumentou de 11.450.265.600 para 12.995.010.560 bytes. Nenhum banco,
objeto nem executável de benchmark foi movido para tmpfs. A nova tentativa
`fleet-caddy-10-smoke-002` mantém os parâmetros e a reserva de 15%; segue em
preparação/validação e ainda não representa aprovação de isolamento/fairness.


### 2026-10-15 15:33 UTC — limite real da topologia de dez projetos

`fleet-caddy-10-smoke-002` foi encerrado automaticamente durante a criação
dos clusters por `host_disk_reserve`, antes de iniciar carga. A limpeza de
todos os recursos próprios foi confirmada. A estimativa anterior de 400 MiB
por projeto não cobria o custo do driver `vfs`; a nova estimativa considera
os tamanhos reais das imagens, duas cópias de camadas nesse driver, margem
por projeto, proxy e orçamento separado para dados PostgreSQL. A repetição
`fleet-caddy-10-preflight-003` foi barrada antes de criar qualquer recurso.
Duas regressões conferem a rejeição antecipada e impedem estimativas com
tamanho inválido ou driver desconhecido; 140 testes passaram em 6,617 s.
[Evidência](results/stress/fleet-vfs-headroom-before-after.json).

Uma tentativa adicional de realocar 4.298.415.884 bytes de caches antigos
não liberou espaço: esses arquivos pertenciam à camada inferior do overlay.
As cópias e seus hashes foram preservados; isso não constitui capacidade de
disco adicional. O perfil declarado de dez projetos precisa de aproximadamente
15,5 GiB inicialmente livres neste filesystem/driver, acima dos cerca de
12,1 GiB disponíveis após a limpeza. Essa célula permanece bloqueada por
recursos; não será substituída por dez bancos no mesmo cluster nem por dados
em tmpfs. A validação real de cinco projetos segue com os limites originais.


### 2026-10-15 15:54 UTC — CI e recusa de comparações incompletas

Os 18 checks do commit `be76404` concluíram com sucesso. A evidência
funcional foi atualizada para esse SHA, com links de cada check; isso não
aprova capacidade nem endurance. A contagem local verificada é de 140 testes
Python em 6,617 s. Contagens Rust não foram inferidas a partir do status.

A reprodução nativa do comparador usou os três resultados iniciais de tráfego
misto de `s3-pass-1-003`, cuja conclusão é `failed`. Comparar esse diretório
consigo mesmo torna as amostras iniciais idênticas: o comparador anterior
retornou aprovação, mesmo com a campanha final interrompida. O comparador
atual recusou a mesma entrada, marcou ambas as conclusões como inválidas e
usou operações úteis. Essa reprodução comprova a correção do gate, sem
afirmar qualquer melhoria de desempenho do produto.
[Evidência](results/stress/comparator-failed-run-before-after.json).

`fleet-caddy-5-smoke-001` concluiu 150 segundos e 35 amostras, mas falhou
no p95 relativo de RLS de `project-3` em `noisy-mixed`; o limite de 20%
permanece intacto e a limpeza foi confirmada. As três referências curtas e
o reload real de proxy passaram; não houve reconciliação final completa
de webhooks porque o gate de fairness encerrou a execução. Uma repetição
com etapas de 300 segundos ainda é necessária. A campanha completa
`fault-pass-1-001` está qualificando a carga antes das falhas reais; as
etapas de 300 segundos a 16 e 32 operações/s passaram. A reconciliação
final de commits continua pendente.


### 2026-10-15 16:06 UTC — campanha completa de falhas

`fault-pass-1-001` passou com 1.580 segundos de carga, 309 amostras e limpeza
confirmada. Foram reconciliadas 59.390 tentativas, 53.049 respostas confirmadas
e 55.988 commits: zero confirmações perdidas e zero identidades/digests
corrompidos. As 2.938 respostas realmente descartadas depois de sucesso do
backend foram encontradas integralmente no banco. Houve 6.155 resultados
ambíguos, dos quais 2.939 foram commits; um deles vinha da categoria de
desconexão anterior à resposta. Nenhuma mutação foi repetida automaticamente;
a repetição explícita da chave da operação retornou 409. As duas contas
preservaram a soma de 2.000.000 e o contador igual ao número de commits.

As etapas de cinco minutos a 16, 32 e 48 iterações/s qualificaram; 64 falhou
por 1.940 chegadas descartadas entre 19.200 (10,104%). Isso não é capacidade
máxima do backend: o gerador Python tem oito operações em voo. As falhas
foram injetadas a 38 iterações/s, 79,167% do patamar qualificado de 48.

Reinício do banco, deadlock, encerramento da aplicação, OOM intencional de
8 MiB com Argon2 inalterado, bytes/inodes cheios e reinício/pausa de RustFS
passaram pelas verificações funcionais e de rejeição. RTOs: banco 0,287 s;
aplicação 1,877 s; OOM 1,509 s; provedor reiniciado 0,472 s; após pausa
0,528 s. O dispositivo de teste retornou 507 `storage_full` por bytes e
500 `internal` por inodes, preservando os bytes existentes, sem publicar
metadados de falha e permitindo upload/read-back posterior. Ocorreram,
respectivamente, 99 e 93 commits de banco durante essas falhas isoladas.

O backup de 3.104.889 bytes ocorreu com escritores ativos. A restauração
local em alvo novo preservou usuário, bytes e rejeição de token do outro
projeto: RTO 2,636 s e RPO superior limitado a 17,997 s. As 646 linhas
de rewind foram intencionais no snapshot restaurado; a origem manteve
todas as confirmações. O gate de multipart vazio foi executado no provedor
real antes da troca de backend; não há arquivo separado dessa contagem.
[Evidência completa e limites](results/stress/fault-pass-1.json).

### 2026-10-15 17:20 UTC — primeiro endurance final em disco

A execução `disk-pass-1-005`, congelada no commit `3203330`, iniciou o trecho
contínuo perto de 17:17 UTC, depois de 3.585 segundos de carga concluída.
As três amostras de cada uma das oito classes, as qualificações e repetições
de cinco minutos até 400 iterações/s, o stress, cinco picos com recuperação
e o reinício planejado passaram. O trecho contínuo oferece 280 iterações/s
(70% de 400); os mesmos processos de aplicação/banco devem permanecer vivos.
A primeira coorte de eventos já foi podada; auth e GC ainda aguardam os ciclos
reais. Nenhum endurance de quatro horas foi aprovado nesta atualização.

O perfil inclui agora três amostras explícitas de escrita a 50 iterações/s e
180 segundos de observação após a carga, para investigar retenção de recursos.
Esse repouso não entra no tempo efetivo nem nas quatro horas contínuas.
O código de produção e o SHA-256 do executável permanecem os do baseline.


### 2026-10-15 17:32 UTC — renovação real e revisão atual

Os 18 checks do commit `3203330` passaram. O checkpoint do cliente contínuo
registrou 876 iterações, uma renovação de acesso e uma operação real de
refresh bem-sucedida, lidas pelo protocolo de contador esparso corrigido.
Isso não aprova o número final de renovações nem as quatro horas: a campanha
continua ativa e a reconciliação final permanece pendente.

O perfil de navegador real está sendo implementado em módulos separados,
com runtime público copiado para uma fixture própria, TLS/NSS isolado,
dois consumidores reais de autenticação e limites explícitos. Até aqui
houve apenas verificação sintática e três testes de proteção contra
traversal/symlinks em metadados do runtime; nenhum resultado de stress
em navegador é apresentado como aprovado. A execução fica para uma janela
entre campanhas, evitando tráfego e cópias pesadas durante o endurance.

### 2026-10-15 17:58 UTC — contabilidade conservadora do tempo efetivo

O novo `bench/load_time.py` lê os intervalos de tráfego concluídos dos
ledgers, valida os limites monotônicos e exclui preparação, warmup, repouso
e intervalos interrompidos. Janelas de geradores sobrepostas contribuem apenas
com sua maior duração de carga, evitando contagem dupla. O resultado é um
limite inferior, não uma certificação de aprovação.

O [checkpoint](results/stress/load-time-checkpoint-001.json) registra 24.119
segundos (6,699722 horas), zero entradas inválidas e zero endurances concluídos.
O trecho contínuo ativo de `disk-pass-1-005` ainda não recebe crédito. Os
148 testes Python relacionados passaram em 5,128 segundos, incluindo cinco
regressões para essa contabilidade. Os 18 checks de `a57fed1` passaram.
Esse CI confirma subconjuntos funcionais; os gates de stress continuam abertos.

### 2026-10-15 19:33 UTC — dois ciclos de retenção, endurance ainda ativo

As duas coortes envelhecidas de sessões, eventos e arquivos foram podadas pelos
loops reais durante a carga. O [checkpoint de 133 minutos](results/stress/disk-pass-1-checkpoint-133min.json)
registra janelas de 15 minutos: medianas de RSS de 45,21 a 46,50 MiB e memória
anônima de 26,92 a 28,20 MiB. As medianas de descritores/sockets continuam em
156/150; as tarefas oscilaram de sete para oito. A inspeção encontrou workers
Tokio adicionais; a recuperação desse pool ainda precisa do repouso final.
Essas observações parciais não certificam ausência de vazamento.

A reconexão do observador preservou aplicação e PostgreSQL, confirmados pelos
mesmos CIDs/PIDs/horários de início e 1.152 amostras contínuas, com gap máximo
de 7,3 segundos. Os 18 checks de `5208714` passaram. Os 148 testes Python
relacionados passaram em 5,784 segundos. O código de produção continua igual
ao baseline; o perfil novo de navegador permanece sem execução nativa.

### 2026-10-15 21:22 UTC — primeiro endurance de quatro horas aprovado

`disk-pass-1-005` passou com 17.985 segundos de carga: 3.585 nas fases prévias
de testes e 14.400 ininterruptos de endurance. Preparação sem tráfego
e os 180 segundos de repouso não entram nessa contagem. O mesmo executável,
aplicação e PostgreSQL permaneceram durante as quatro horas. Foram 3.094
amostras, 2.421 durante o endurance; gap máximo global de 12 segundos.

A 280 iterações/s, foram observadas 4.031.806 operações úteis, zero chegadas
descartadas, zero falhas inesperadas e zero rejeições por sobrecarga. Os p95/p99
em ms foram: leitura 7,69/16,34; RLS 6,40/14,66; login 50,20/76,01;
refresh 19,09/33,65; storage 17,96/31,66; escrita 15,03/28,29. São estimativas
de histogramas nativos, sob as cotas e o perfil declarados; não capacidade máxima.
O canário contínuo fez 16 renovações reais. As duas coortes de retenção passaram.

A reconciliação encontrou 248.381 eventos, sem perdas, extras ou duplicatas;
129 objetos com bytes conferidos, mais 10.000 objetos vazios físicos de seed,
sem chaves órfãs. Fila final zero, sem transações ociosas nem locks esperando.
O repouso voltou aos mesmos 24 descritores, quatro tarefas e 18 sockets das
cinco recuperações anteriores. RSS máximo global 49,29 MiB; PostgreSQL chegou
a 429 MiB em disco. Os componentes não tiveram OOM. Limpeza confirmada.
[Evidência completa](results/stress/disk-pass-1.json).

O [segundo checkpoint de tempo](results/stress/load-time-checkpoint-002.json)
atingiu 38.519 segundos, **10h41m59s efetivas concluídas**, sem contar intervalos
interrompidos. Isso cumpre o mínimo acumulado de oito horas, mas não conclui
G05: faltam S3 contínuo, segunda passada e os demais cenários/bloqueios visíveis.
A retenção de memória anônima precisa de comparação e evidência de alocação;
recuperar os owners observáveis não prova ausência absoluta de vazamento.

### 2026-10-15 21:37 UTC — validação nativa do perfil de navegador

As primeiras tentativas foram preservadas, todas com limpeza confirmada.
`001` rejeitou o alias ELF `/lib64`; a validação agora resolve a biblioteca e
também impede symlinks que escapam das raízes aprovadas. `002` encontrou uma
extensão Tcl/Tk opcional sem dependências instaladas; esse perfil headless
exclui somente o toolkit não utilizado. `003` revelou que o executável público
Node do Playwright era um symlink omitido pelo empacotamento; agora seus bytes
são copiados explicitamente, mantendo links genéricos recusados.

`004/005` abortaram no controle TLS. O diagnóstico nativo limitado confirmou
`libsqlite3.so.0` ausente no módulo NSS carregado dinamicamente. A correção
inclui suas dependências reais; o controle de CA não confiável passou em `006`.
Os 152 testes relacionados passaram em 5,563 segundos, incluindo resolução
segura de bibliotecas e o fechamento de dependências do módulo ELF NSS real.

`006` também revelou um oráculo offline incorreto: o SDK documenta duas novas
tentativas de leitura. O perfil fixa esse contrato, exige exatamente três
tentativas GET, uma tentativa POST e ausência de commit/replay ao reconectar;
não houve mudança do SDK. `007` passou desses controles, conflitos reais de
arquivo e read-back, mas parou em um seletor de fechamento ambíguo. A prévia
agora fecha por Escape, com confirmação de que o diálogo desapareceu.

`008` é diagnóstico em andamento. A revisão adicional fortaleceu a próxima
execução para conferir todos os 20 WeakRefs por aba/ciclo e alternar sempre a
tabela, evitando medir dados já visíveis como uma nova resposta. Não há ainda
aprovação do perfil completo de navegador nem do p95 visual final.

### 2026-10-15 22:02 UTC — navegador real aprovado e S3 iniciado

`browser-pass-1-009` passou pelo perfil fortalecido, em fixtures novas:
três abas, 511 navegações com tabela realmente diferente e aba visível,
p95 de 399,4 ms para os dados autoritativos e dois frames, abaixo de 1 segundo.
Foram verificados todos os 300 clientes temporários descartados por WeakRef/GC,
seis conflitos reais de upload de 1 MiB e três substituições com read-back e
digest corretos, cinco prévias e cancelamento de duas respostas reais 200.

A sessão foi observada por 1.082 segundos, com TTL real de 900: renovação
automática, token inicial expirado rejeitado com 401 e logout entre abas com
refresh revogado rejeitado com 400. As 2.499 leituras SDK mantiveram RLS e
identidade; os dois consumidores de autenticação responderam. Offline houve
três tentativas GET, uma POST e nenhuma mutação/repetição após reconectar.
Zero erros de página e zero solicitações externas. Houve 219 amostras e limpeza
confirmada. [Evidência](results/stress/browser-pass-1.json). O intervalo contém
repousos e controles e não recebe crédito separado no contador conservador de
oito horas. A segunda passada do navegador continua necessária.

`008` foi interrompido de forma controlada para substituir os oráculos fracos;
seus cinco ciclos e sua limpeza foram preservados, sem aprovação do perfil.
A execução nova confirma todos os WeakRefs e registra o feedback antes de
qualquer reprovação. Não se atribui melhoria de produto à correção do executor.

`s3-pass-1-004` iniciou depois da limpeza do navegador, com RustFS real,
1.024 MiB para o provedor, as oito classes, escrita nominal explícita de 50/s,
WAL de 256 MiB, teto de qualificação de 400 e fração de 70%. O endurance
solicitado é de 14.400 segundos após as fases de qualificação/picos. Nesta
atualização ele está nas fases prévias: não há quatro horas S3 aprovadas.
Os 18 checks de `f46ab4b` passaram.

### 2026-10-16 01:35 UTC — controle adicional de arenas de threads

O observador opcional de alocação recebeu um controle nativo que aloca e toca
32 MiB em uma thread secundária, abaixo do limiar de mmap definido somente
no controle, e depois libera todos os blocos. O teste confirmou aumento nos
bytes em uso e nas arenas, sem atribuir o aumento a mmap, e retorno após free.
Passou em 1,300 segundo; wrapper 1,420 segundo, CPU de filhos 0,320 segundo,
RSS máximo 33.408 KiB. Foi isolado na CPU 4, em baixa prioridade. Isso valida
a glibc do controlador; a glibc da imagem de staging ainda precisa do mesmo
controle antes de interpretar um diagnóstico instrumentado do backend.

O preload é desenvolvimento opcional, com um descritor e uma thread adicionais,
amostragem limitada e evidência numérica. Não altera o executável de produção,
não chama malloc_trim nem ajusta arenas do backend. O comparador recusa esses
perfis para certificação de desempenho comum. Os cinco controles nativos
anteriores e seis testes de evidência/comparação passaram; não há ainda
campanha instrumentada de backend concluída. Os 18 checks de d016e4c passaram.

O S3 contínuo segue em execução desde aproximadamente 23:05 UTC: 2h30 neste
checkpoint, sem OOM ou erro de assinatura e com as duas coortes de retenção
removidas. A campanha só poderá ser aprovada após quatro horas, repouso final,
reconciliação e limpeza.

### 2026-10-16 03:08 UTC — primeiro S3 contínuo aprovado

`s3-pass-1-004` passou: 14.400 segundos ininterruptos após as fases de carga,
17.985 segundos registrados pelo executor, 2.934 amostras (2.297 no endurance)
e gap máximo de 11,5 segundos. O contador conservador exclui também o warmup:
[o checkpoint 003](results/stress/load-time-checkpoint-003.json) totaliza
56.489 segundos, **15h41m29s efetivas concluídas**, sem preparação, repousos
ou intervalos incompletos. O binário continua 75c44c85, fonte 2efede8c.

A taxa de 280/s corresponde a 70% dos 400/s qualificados em passos repetidos
de cinco minutos, sem alegar capacidade máxima. Foram 4.031.965 operações
úteis, zero drops, erros inesperados ou rejeições por sobrecarga. Os p95/p99
em ms foram: leitura 5,72/11,08; RLS 4,71/10,62; login 42,03/56,33;
refresh 16,10/24,20; storage 24,13/37,08; escrita 12,15/19,76. Houve
16 renovações reais e duas coortes efetivamente removidas por retenção/GC.

A reconciliação encontrou 249.824 eventos/deliveries únicos, sem perda, extra
ou duplicata; 129 objetos com bytes conferidos mais 10.000 objetos físicos
vazios, zero multipart abandonado e zero chave órfã. A fila zerou; o banco
terminou com 15 conexões, zero transação ociosa e zero lock esperando.
Aplicação e PostgreSQL preservaram CIDs/PIDs/inícios. A reconexão breve do
terminal não reiniciou componentes; seu checkpoint foi preservado.

Após o repouso, aplicação voltou a 24 descritores, quatro tarefas e 18 sockets.
O PID1 do RustFS caiu de 114 descritores/176 tarefas/99 sockets no começo
para 18/172/6. A memória anônima do provedor, entretanto, passou de mediana
de 353,4 MiB na primeira janela a 415,4 MiB na última; ficou em cerca de
407 MiB no repouso. Não se atribui esse crescimento a cache ou allocator
sem evidência de ownership. A fonte associada à revisão declarada na imagem
usa MiMalloc; isso não equivale a prova de alocação do executável fornecido.
O diagnóstico glibc do Nelcota não mede esse allocator do provedor.

Zero OOM, RSS máximo da aplicação 49,875 MiB, banco até 431 MiB em disco,
limpeza confirmada. [Evidência](results/stress/s3-pass-1.json). A segunda
passada independente e os demais critérios de G05 permanecem pendentes.

Os controles malloc/free de 64 MiB e arena secundária de 32 MiB passaram
também na imagem Ubuntu exata do staging, isolados, sem rede, 96 MiB RAM,
0,125 CPU, PID/capacidade/duração limitados e watchdog independente. Ambos
produziram 11 amostras e retornaram ao uso inicial; nenhuma OOM e limpeza
confirmada. [Evidência](results/stress/allocation-target-controls.json).
O smoke instrumentado do backend está em execução; não certifica desempenho.

### 2026-10-16 04:31 UTC — uma hora de diagnóstico de alocação concluída

`allocation-smoke-001` passou com 150 segundos de carga e 66 amostras;
`allocation-disk-hour-001` passou com 3.600 segundos contínuos, 4.035 segundos
registrados e 770 amostras, gap máximo 11 segundos. Ambos mantêm o binário
de produção e adicionam somente o observador opcional. A preparação, o repouso
e a instrumentação ficam explícitos; o comparador recusa esse perfil para
certificação de regressão comum. Não substituem a segunda passada de quatro horas.

Na hora contínua foram 720 amostras de allocator em quatro janelas de 15 minutos.
As medianas em uso foram 4,846 / 4,899 / 4,936 / 4,959 MiB. Picos transitórios
chegaram a aproximadamente 24 MiB e foram liberados. No final, 2.067.920 bytes
continuavam em uso, 25.166.384 bytes estavam livres nas arenas e as arenas
totalizavam 27.234.304 bytes. Isso distingue retenção do allocator dos bytes
contados em uso nesta campanha; não é censo de objetos Rust nem prova absoluta
de ausência de vazamento. O perfil curto não executa o canário especial de
expiração nem duas coortes de retenção: esses cenários não recebem aprovação
a partir deste diagnóstico. Limpeza confirmada.
[Evidência](results/stress/allocation-disk-hour.json).

### 2026-10-16 04:36 UTC — espaço recuperado sem reduzir reservas

A inspeção encontrou artefatos de compilação recompiláveis com todos os seus
hardlinks dentro de `target/debug`; foram excluídos executáveis ativos e o
baseline de testes. Um arquivo privado limitado a 1 GiB preservou 121 caminhos,
com hashes SHA-256 verificados após descompressão antes de remover os originais.
A liberação efetivamente medida foi de 3.560,39 MiB em 54,12 segundos. O backup
temporário contém somente cache de compilação; sua restauração é obrigatória
antes dos checks Rust e será registrada separadamente. Dados e objetos de
staging continuam em disco; nenhuma evidência irreproduzível foi transferida
para tmpfs. O script de restauração já está preparado.

Também foram comprimidos de forma reversível 577 arquivos privados de
observação/metrics de execuções concluídas e limpas, sem modificar os resumos
de fases, ledgers de eventos, falhas ou veredictos. Cada arquivo foi
descomprimido e conferido por tamanho e SHA-256 antes de remover a cópia
sem compressão; liberação adicional medida: 314,78 MiB. Os bytes originais
estão em `.gz` no próprio diretório persistente da execução, e o índice
privado registra os hashes e os metadados para leitura/restauração.

A guarda real inclui 8.440 MiB de startup para dez projetos, 2.048 MiB para
dados, outro buffer de 512 MiB e a reserva de 15% do filesystem. Nenhum deles
foi reduzido. O preflight de `fleet-caddy-10-pass-1-001` passou e seu manifesto
de ownership foi criado para 23 containers/11 volumes, preparação máxima
de 900 segundos e duração total de 7.200 segundos. O perfil completo usa
passos de 300 segundos, capacidade/saturação, webhooks e Caddy com TLS local
verificado. Nesta atualização está provisionando; isolamento, capacidade e
fairness ainda não foram aprovados.
[Evidência do espaço](results/stress/staging-cache-and-diagnostics-space.json).

### 2026-10-16 05:16 UTC — dez projetos: reprovação nominal preservada

`fleet-caddy-10-pass-1-001` provisionou dez aplicações/bancos independentes,
passou os 90 rejeites direcionados de credenciais cruzadas e executou 1.590
segundos de carga, com 124 observações e limpeza confirmada. As qualificações
de 8/16/24 iterações/s por projeto passaram; 32/s reprovou refresh em oito
projetos. A primeira medição nominal a 19/s reprovou refresh em dois projetos:
p95 233,01 e 234,27 ms, contra 200 ms. Cada projeto entregou 5.701 iterações,
zero drops, falhas inesperadas ou rejeições por sobrecarga. Não se aprova
capacidade sustentável a partir de um passo isolado que não se repetiu em
serviço nominal. Fairness, demais amostras e reconciliação final não foram
atingidas. [Evidência da falha](results/stress/fleet-10-default-cpu-period.json).

As observações tiveram mediana de 16,09 segundos e máximo de 24,30: abaixo
do limite crítico de 30 segundos, mas o alvo de cinco segundos não foi
atingido. Não houve OOM, violações de assinatura ou locks esperando observados.
A coleta de dez projetos precisa manter esse custo/limitação visível.

Cada aplicação/banco tinha quota de 10 ms a cada 100 ms (0,1 CPU). No trecho
final observado de 170,5 segundos, aplicações sofreram throttling em cerca
de 9% a 18% dos períodos. A hipótese é que bursts de hashing e outras tarefas
consomem a quota cedo e prolongam a espera da API mesmo com uso médio menor.
A identificação não implica uma correção no código de produção.

O executor agora aceita `--cpu-period-micros 10000` para investigar a mesma
fatia de CPU em janelas menores. O padrão segue em 100 ms; a soma das cotas
de apps/bancos permanece a mesma. Seis controles nativos de kernel passaram
para 1/5/10 projetos: em dez projetos, 1.000 microssegundos a cada 10.000
contra 10.000 a cada 100.000. Sem rede, 32 MiB RAM, PID/capabilities/root/duração
limitados, watchdog e limpeza confirmada.
[Evidência](results/stress/cpu-policy-native-controls.json).

Doze testes de política/comparação passaram; o conjunto relacionado de 138
testes passou em 10,290 segundos. O comparador recusa períodos diferentes e
valores ausentes, não finitos ou inválidos. Nenhum SLO foi relaxado e nenhum
resultado anterior reprovado foi reescrito. O efeito da configuração sob
carga ainda requer campanha nova no artefato congelado.

### 2026-10-16 05:30 UTC — fechamento dos leitores de evidência SQLite

A segunda compactação preservou por SHA-256 os bytes de 44 arquivos concluídos,
incluindo 35 ledgers SQLite fechados, após `PRAGMA integrity_check` em modo
read-only/immutable. Suas cópias `.sqlite.gz` ficam no diretório persistente
da respectiva execução. O índice e as falhas anteriores foram mantidos.

A medida imediata de espaço nesse utilitário ficou negativa porque o contexto
de transação do SQLite não fecha a conexão: descritores ainda seguravam
arquivos já substituídos por cópias compactadas. O encerramento do processo
liberou os leitores. O índice preserva a medida original e um checkpoint
posterior registra o espaço após a saída, sem reescrever a observação.

O helper de verificação agora fecha explicitamente cada conexão, inclusive
se a consulta falha, e rejeita journals e symlinks. O controle nativo, com
35 verificações e GC desativado em ambos os casos, reproduziu quatro para
39 descritores com o contexto antigo; a versão corrigida permaneceu em
quatro. Os bytes do banco não mudaram. Quatro regressões passaram, cobrindo
fechamento repetido, erro de integridade, journal ativo e redirecionamento
por symlink. [Antes/depois](results/stress/sqlite-archive-owner-before-after.json).
É uma correção do utilitário de evidência do staging; não foi uma falha do
processo Rust ou dos bancos da aplicação. A próxima compactação utilizará
o helper corrigido. O espaço continua sujeito ao preflight e à reserva de 15%.

### 2026-10-16 05:59 UTC — watchdog real e gargalo do observador

`fleet-caddy-10-period10-pass-1-001` reprovou: após 900 segundos de carga
concluída e 68 amostras, uma observação incompleta ultrapassou os 30 segundos
e o watchdog independente interrompeu o tráfego. A limpeza foi confirmada.
Os três passos concluídos de 8/16/24 por projeto também reprovaram o SLO
de refresh; o período menor não recebeu aprovação. Não se atribui melhoria
a essa configuração. [Evidência](results/stress/fleet-10-serial-observer-stop.json).

O coletor anterior executava em série 22 leituras de cgroup, stats do Docker
e consultas/tamanhos de dez bancos. O custo real excedeu o intervalo
de observação. A correção limita o paralelismo a quatro probes, usa somente
builtins do shell para os arquivos de cgroup (evitando um processo cat por
arquivo) e registra duração total e de cada probe. O protocolo novo é explícito
e não pode ser comparado como equivalente ao serial. O caminho padrão dos
diagnósticos de projeto único continua com o protocolo anterior.

Uma falha em qualquer probe sinaliza a parada antes de aguardar os outros
owners; o pool termina suas threads e nunca publica um snapshot parcial.
O limite crítico continua em 30 segundos. Dezessete testes focados passaram,
incluindo quatro probes avançando ao mesmo tempo, limite máximo de quatro,
erro sinalizado enquanto outro owner ainda termina, ausência de threads
retidas e leitura nativa do cgroup com PATH sem nenhum executável externo.

O erro secundário URLError vinha da tentativa de reter métricas depois que
o watchdog removia o servidor. A retenção agora registra o erro secundário
e preserva a exceção original. Se não houver falha anterior, a ausência
do snapshot continua reprovando a campanha. Duas regressões cobrem esses
casos. O observador corrigido ainda precisa da substituição nativa do perfil.

### 2026-10-16 06:26 UTC — observador validado em serviços reais

`fleet-observer-1-smoke-001` passou com 120 segundos de carga, 27 amostras,
protocolo paralelo de quatro workers, aplicação/PostgreSQL reais, Caddy com
TLS verificado, falha de reload injetada, eventos reconciliados e limpeza.
A mediana entre amostras foi 5.000 s e o máximo 5.049 s;
a duração mediana da coleta foi 2.570 s.
É validação nativa do observador em um projeto; não aprova capacidade nem
fairness de dez projetos. [Evidência](results/stress/fleet-parallel-observer-smoke.json).

O executor também passa a registrar todas as falhas não críticas de fairness
e continuar os demais cenários de interferência e a reconciliação final.
Qualquer falha mantém o resultado global reprovado; o comparador exige
a lista de falhas vazia independentemente do completion. Um baseline
balanceado inválido interrompe antes dessa fase, e violações de integridade,
ownership ou recursos continuam interrompendo o tráfego. Os testes cobrem
os cinco cenários após uma reprovação relativa, interrupção em erro crítico
e rejeição de evidência incompleta.

A próxima tentativa completa de dez projetos usará novamente o período
padrão de 100 ms, observador paralelo e teto de qualificação declarado de
8 iterações/s por projeto. Esse ponto mais baixo será requalificado em
passos de 300 segundos após a falha nominal a 19/s; os SLOs absolutos e
as exigências de fairness continuam iguais. Não se apresenta esse teto
como capacidade máxima nem o período experimental menor como melhoria.

O conjunto relacionado após o ajuste do fluxo e do comparador passou: **154
testes em 10,343 segundos**. O snapshot parcial de um cenário não substitui
a aprovação da campanha inteira. Checkpoint registrado em 2026-10-16 06:30 UTC.

### 2026-10-16 08:05 UTC — interferência completa e descartes do scheduler

`fleet-caddy-10-bounded-pass-1-001` terminou reprovado, com 3.990 segundos
de carga concluída, 1.042 amostras e limpeza confirmada. O observador corrigido
atingiu mediana de 5,000 segundos, máximo de 6,235 e nenhuma ocorrência de OOM.
Os passos de 2/4/6 iterações/s por projeto passaram; 8/s reprovou refresh.
A taxa nominal adaptativa foi 4/s, não o argumento de fallback de 6/s.
As três amostras balanceadas e a falha/recuperação real de reload do Caddy
passaram. Isso não certifica o restante da topologia.

Os cinco cenários concentrando 80% da carga oferecida no projeto zero
reprovaram o limite relativo de p95 dos projetos saudáveis: 23/29/32/33/33
achados para mixed/login/read/storage/write, respectivamente. A drenagem
de webhooks também reprovou o prazo existente de 30 segundos; a última
observação durante a drenagem ainda registrava 5.834 pendências no projeto
zero e zero nos outros nove. A reconciliação final de identidades e marcadores
não foi alcançada; não se declara ausência de perda a partir dessa campanha.
[Evidência por fase/projeto](results/stress/fleet-10-bounded-pass-1.json).

A inspeção do snapshot bruto confirmou um defeito adicional do executor:
`dropped_iterations` é emitido pelo scheduler com `scenario=project-0` e
`project=single`, pois uma chegada descartada não executa VU para receber
a tag dinâmica. O filtro antigo por projeto inventava zero. Na escrita
concentrada foram 35.226 descartes reais e 7.975 iterações concluídas,
contra o resumo anterior de zero descartes. A consulta corrigida usa o
cenário somente para esse contador e mantém o filtro de operações/latências.
Quatro regressões passaram, incluindo isolamento por projeto/fase e exclusão
do canário na campanha de projeto único. O caminho de projeto único já
filtrava por cenário e permanece equivalente. Os bytes brutos e os resultados
históricos não foram reescritos; a evidência pública distingue o valor
incorreto original do total recuperado. Os resultados antigos de frota
não recebem aprovação de geração de tráfego a partir do contador incorreto.

Uma segunda execução fresca do mesmo perfil congelado anterior está em curso
para verificar a reprodução dos problemas de serviço/interferência. A correção
do contador requer controle nativo e validação nos próximos perfis; não torna
a campanha em curso uma execução do executor corrigido. Nenhum prazo ou SLO
foi ampliado. O limite de 128 VUs do gerador é agregado entre os dez projetos;
sob a carga concentrada o gerador não entrega toda a taxa oferecida.

### 2026-10-16 09:39 UTC — reprodução independente e controle nativo

`fleet-caddy-10-bounded-pass-2-001` repetiu o perfil anterior em dez clusters
novos: 3.990 segundos de carga concluída, 1.039 amostras, mediana entre
amostras de 5,000 segundos, máximo de 5,887 e zero OOM observado. Qualificou
novamente 6/s por projeto e adotou 4/s nominal. As três amostras balanceadas
e o reload passaram; os cinco cenários de interferência reprovaram
17/15/21/19/36 gates relativos. O prazo de drenagem de 30 segundos voltou
a reprovar; a última observação de drenagem tinha 7.010 pendências no projeto
zero. A escrita concentrada concluiu 9.204 iterações e teve 33.997 descartes
do scheduler recuperados do snapshot bruto. Não houve reconciliação final
de identidades/marcadores. Os 23 contêineres e 11 volumes foram removidos,
com limpeza confirmada. Essa reprodução confirma uma reprovação do perfil
com CPU fracionada e da entrega do gerador; não identifica por si só um
defeito de produção ou uma capacidade sustentável máxima.
[Evidência da repetição](results/stress/fleet-10-bounded-pass-2.json).

O controle seguinte usou k6 e Prometheus reais, duas chegadas controladas
com um VU por cenário e iterações artificiais de sleep. O filtro antigo por
tag de projeto retornou zero para ambos; o filtro corrigido por cenário
recuperou 127 e 51 descartes, com 34 e 30 iterações concluídas. Os contadores
brutos realmente tinham `project=single`. O controle durou 23,804 segundos,
com prazo independente de 180 segundos, quotas de 0,25 CPU/128 MiB por
contêiner, sem tráfego HTTP de aplicação e limpeza confirmada. Ele valida
o contador, não capacidade da aplicação.
[Controle nativo](results/stress/scheduler-drop-native-control.json).

O CI do primeiro commit da correção reprovou a asserção antiga que exigia
a tag dinâmica em todas as consultas. A regressão foi atualizada para exigir
isolamento por cenário nos descartes e por projeto nas operações. Cinco
testes focados passaram, seguidos por **188 testes Python em 12,445 segundos**.
Os 18 checks de CI de `621dab9` passaram. Uma tentativa local com nome de
classe incorreto não executou o teste e foi corrigida; não entra na contagem.

Os 121 caminhos de cache recompilável arquivados foram restaurados em
20,043 segundos, com SHA-256, modos, mtimes e hardlinks conferidos antes
dos checks locais. Nenhum arquivo existente foi sobrescrito.
[Restauração](results/stress/staging-cache-restore.json).
O checkpoint 005 soma **71.234 segundos (19h47m14s)** de carga efetiva
conservadora, sem preparação, rest, intervalos interrompidos ou duplicação
de janelas concorrentes. Esse total inclui testes reprovados com tráfego
real e não implica aprovação de seus critérios.

### 2026-10-16 11:38 UTC — errata: descartes esparsos no endurance

A revisão dos snapshots e logs confirmou outro defeito de medição. k6 só
publica o contador de descartes quando há descartes; a consulta instantânea
deixa de enxergar a série após cinco minutos, e o Prometheus da fixture
retém uma hora. Consultar apenas ao final de quatro horas pode fabricar zero.
Os logs dos primeiros endurances de disco/S3 registram falta de VUs, embora
seus snapshots finais não tenham nenhuma série de descartes. **Os totais
históricos de zero descartes não estão sustentados e ficam retirados.**
Seus totais exatos não podem ser recuperados após o teardown do Prometheus.
A continuidade física de quatro horas, os contadores de trabalho útil,
os bytes/eventos reconciliados e os demais resultados originais continuam
registrados com seu escopo; não se aprova o protocolo corrigido a partir deles.

O segundo disco iniciou endurance aproximadamente às 10:43 UTC, no executor
congelado anterior. Uma consulta enquanto todo seu prefixo ainda cabia na
retenção recuperou **607 descartes**, sem reset observado. Um observador
suplementar somente de leitura acompanha esse contador para investigar a
falha sem alterar o tráfego ou reiniciar o serviço. Ele começou durante a
campanha e não a transforma em uma execução do executor corrigido. A primeira
tentativa de iniciar esse observador falhou antes da coleta: o limite de
address space herdado impedia o cliente Docker em Go de reservar sua arena
virtual. A tentativa seguinte preservou esse diagnóstico e iniciou a coleta
com estado/payloads limitados, afinidade e prazo próprios.

A correção permanente abre o contador antes de cada gerador, coleta o total
por cenário durante as observações, conserva os valores quando a série
expira e fecha a cobertura após a saída do gerador. Estado limitado a 128
fases/11 cenários, lookback de 120 segundos e lacuna máxima de 30 segundos;
reset, regressão, número inválido ou cenário desconhecido reprova a coleta.
Os relatórios incluem a cobertura fechada e os comparadores passam a exigi-la.
Os SLOs, o limite de 1% de descartes e a entrega mínima de 99% permanecem iguais.

O controle real do comparador antigo aprovava oito workloads ao comparar
o primeiro disco consigo mesmo, apesar da ausência dessa evidência. O
comparador corrigido reprovou os oito e marcou ambas as evidências inválidas.
**47 testes focados passaram em 1,020 segundo**, incluindo simulação de quatro
horas com um descarte esparso inicial, resets, lacunas, isolamento por cenário,
contadores inconsistentes e rejeição por ambos os comparadores. A validação
nativa do novo protocolo e as campanhas de substituição ainda estão pendentes;
as quatro horas em curso não serão promovidas como se tivessem começado nele.
[Achado e reprodução](results/stress/sparse-drop-staleness-finding.json).

### 2026-10-16 14:47 UTC — segundo disco concluído e falha de controle reproduzida

`disk-pass-2-001` concluiu 14.400 segundos contínuos no executor congelado
`028f6e7`, com os mesmos processos de aplicação/PostgreSQL, 3.134 amostras
e lacuna máxima de 13,7 segundos. Foram 4.030.874 operações úteis, 16 renovações
reais e duas coortes removidas pelos pruners. A reconciliação contou 249.795
eventos e entregas, sem ausentes, extras ou duplicatas, e 129 objetos verificados
além dos 10.000 objetos vazios reais. A fila drenou a zero; o plateau final
voltou a 24 descritores/quatro tarefas/18 sockets. O banco tinha 15 conexões,
zero transações idle e zero esperas por locks. Limpeza confirmada.

O resultado bruto antigo continua `passed`, mas **a aprovação do protocolo de
chegadas fica reprovada**: ele reportou zero descartes, enquanto o observador
suplementar reteve 1.127. Uma observação real mostrou tanto a consulta instantânea
quanto a janela de uma hora em zero enquanto esse total permanecia preservado.
A coleta suplementar começou durante a fase e teve uma lacuna inicial de
124,49 segundos; não substitui a cobertura de 30 segundos desde o início
exigida pelo executor corrigido. [Diagnóstico](results/stress/disk-pass-2-diagnostic.json).

O checkpoint 006 soma **89.204 segundos (24h46m44s)** de carga concluída,
excluindo preparação, warmup, repouso, intervalos interrompidos e duplicação
de janelas concorrentes. São horas efetivas de testes, incluindo reprovações,
e não uma certificação do G05.
[Contagem auditável](results/stress/load-time-checkpoint-006.json).

O controle nativo novo de contadores reprovou no CI de `c49470b`. A reprodução
local preservada também reprovou: k6 saiu ao atingir taxa zero e não manteve
a cauda sem chegadas por 130 segundos. O teste então rejeitou corretamente
a cobertura inferior aos 136 segundos exigidos. A regressão de relógio virtual
falha no código anterior; após a correção, 15 testes focados passaram em
0,207 segundo. O controlador agora conserva observação limitada de contadores,
volume e reservas por pelo menos 130 segundos após o flush final do gerador.
Esse repouso artificial não entra na contagem de stress da aplicação. A
repetição real do controle permanece pendente neste checkpoint; os logs e
a tentativa reprovada foram preservados, com limpeza confirmada.

A repetição `sparse-drop-native-control-002`, em `d4ea5bc`, passou com k6 e
Prometheus reais: 538 descartes retidos, consulta instantânea final zero e
janela final de 120 segundos zero. A cobertura fechada teve 30 observações,
140,012 segundos e lacuna máxima de 6,491 segundos. Foram 130,012 segundos
de observação após a saída do gerador; limpeza confirmada. Essa duração
artificial continua excluída das horas efetivas de stress da aplicação.
[A falha e a repetição](results/stress/native-arrival-control-before-after.json).

A descoberta completa de regressões Python passou: **211 testes em 12,813
segundos**, após a correção do controle (`python-checks-015.log`). O check
`native-lint` de `d4ea5bc` também passou no CI; os demais checks desse SHA ainda
estavam em andamento neste registro. A tentativa 2 de Rust no SHA anterior
`3e3e92b` terminou com sucesso; a falha original e sua causa não identificada
continuam preservadas, sem declarar que um defeito Rust foi corrigido.

### 2026-10-16 15:08 UTC — integração e fixture maior

`large-disk-counter-smoke-001`, congelado em `196e0e4`, passou com um milhão
de linhas em cada tabela de produtos/privada e payload de 1.048.576 bytes.
Foram 330 segundos de carga concluída após excluir o warmup de 15 segundos,
79 amostras e lacuna máxima de 11,1 segundos. As 41 fases concluídas têm
cobertura fechada do contador corrigido, lacuna máxima de 8,857 segundos
e zero descartes retidos. A validação final conferiu 29 objetos publicados,
30.408.704 bytes físicos, além dos 10.000 objetos vazios, sem chaves órfãs;
1.660 eventos/entregas, sem perdas, extras ou duplicatas. Pico de banco
631 MiB, armazenamento 30 MiB e zero OOM observado. Limpeza confirmada.
As calibrações de cinco segundos e soak de 60 segundos são funcionais,
não qualificam capacidade de cinco minutos ou endurance de quatro horas.
[Evidência do smoke](results/stress/large-disk-smoke.json).

Os **18 checks de CI passaram em `196e0e4`**, incluindo o controle nativo
corrigido. O PR continua em rascunho, sem merge ou deploy. Esse resultado
não aprova os cenários de stress ainda pendentes.
[Checks e URLs](results/stress/ci-196e0e4.json).

A campanha `fleet-caddy-1-final-pass-1-001` iniciou a primeira passagem
completa do protocolo corrigido em um projeto independente. Perfil pré-declarado:
etapas de 300 segundos, teto provisório de 32 iterações/s por projeto,
Caddy com TLS local verificado, webhooks reais e orçamento de banco de
2.048 MiB. As campanhas permanecem sequenciais nos limites deste ambiente.

### 2026-10-16 15:45–15:49 UTC — primeira campanha completa de um projeto

`fleet-caddy-1-final-pass-1-001`, congelado em `196e0e4`, passou com 2.490
segundos de carga, 506 amostras e lacuna máxima de 5,119 segundos. Qualificou
32 iterações/s como limite inferior dentro do teto declarado e usou 25/s
nominal; as três amostras balanceadas de 300 segundos, sobrecargas e o reload
real do Caddy passaram. A configuração inválida foi rejeitada; a restauração
recuperou sondas positivas e 401 em 0,043 segundo, mantendo o mesmo processo
do proxy e verificação TLS. Foram 2.928 eventos/entregas conciliados, sem
ausentes, extras, duplicatas, pendências ou erros de assinatura. Zero OOM
observado e limpeza confirmada. A segunda passagem independente permanece
necessária. [Evidência](results/stress/fleet-1-final-pass-1.json).

O checkpoint 007 soma **92.024 segundos (25h33m44s)** de carga efetiva
conservadora. Controles artificiais e preparação não foram adicionados.
[Contagem](results/stress/load-time-checkpoint-007.json).

Foi preparado um perfil experimental de CPU para investigar as reprovações
com quotas fracionadas: aplicações compartilham um único CPU lógico e bancos
compartilham outro, mantendo o gerador em um terceiro. Cada contêiner pode
usar a folga do seu grupo, mas a afinidade limita o grupo inteiro a um CPU.
A soma das quotas individuais é explicitamente sobrealocada dentro desse
único CPU; não é aumento oculto do orçamento físico do grupo. Memória, disco,
SLOs, 90% de throughput, 20% de p95 e drenagem de 30 segundos permanecem iguais.
É um novo perfil com baseline e repetição próprios, sem promover as duas
reprovações anteriores de dez projetos ou declarar correção de produção.

A implementação e seus guards passaram **27 testes focados em 0,032 segundo**
e **220 testes Python em 12,759 segundos**. O leitor do relatório real anterior
de um projeto passou por normalização da geometria exata 1/1; a autocomparação
foi somente um controle de compatibilidade, sem substituir campanhas distintas.
O commit `93d6723` mantém o perfil de quota como padrão, confere afinidade e
quota reais de todos os contêineres antes de oferecer tráfego e exige censo
completo nos comparadores de novos perfis.

O primeiro lançamento do controle nativo foi recusado antes de criar recursos:
o controlador já estava fixado em um CPU ao verificar o ambiente. A invocação
foi corrigida para registrar os CPUs disponíveis antes de fixar somente o
controlador no CPU reservado. A tentativa continua preservada. A repetição
`cpu-sharing-native-002` passou em 36,085 segundos, com 32 contêineres
auxiliares descartáveis nas topologias 1/5/10, no máximo 20 simultâneos.
Cada um teve afinidade/quota reais conferidas, 32 MiB, 16 processos, rede
desabilitada, root somente leitura e nenhuma capability; limpeza confirmada.
Esses auxiliares inertes validam recursos, sem substituir aplicações/bancos
reais ou acrescentar horas de stress.
[Controle nativo](results/stress/cpu-sharing-native-controls.json).

As próximas campanhas e seus parâmetros foram
[pré-declarados](results/stress/final-campaign-plan.json). As substituições
ordinárias de disco/S3 terão duas passagens frescas de quatro horas por backend
e amostras nominais de 60 segundos, definidas antes das novas medições para
caracterizar melhor a variação; os limites de regressão de 10% permanecem iguais.
O smoke de cinco aplicações/bancos reais com compartilhamento iniciou após
o controle nativo; seu resultado ainda está pendente neste registro.

### 2026-10-16 15:57–16:07 UTC — smoke de cinco projetos reprovado

`fleet-caddy-5-sharing-smoke-001` concluiu 270 segundos de carga, 85 amostras
e limpeza confirmada, mas **reprovou** os cinco cenários concentrados: 6/22/14/11/8
achados de gate em mixed/login/read/storage/write. Sob login intenso, RLS
teve p95 de 217,638 ms em três projetos saudáveis, acima do limite absoluto
de 200 ms. Cada saudável teve 61 operações úteis e 12 observações de RLS;
os histogramas nativos têm rótulos de projeto/cenário distintos e contêm
o outlier real, sem mistura de métricas entre projetos. Não foi atribuída
causa raiz ou declarada correção de produção.

O kernel confirmou o novo limite de cada um dos dez contêineres reais. Não
houve OOM, falha de recursos, erro de assinatura ou perda/duplicação de evento:
1.129 identidades/entregas conciliadas e zero pendências. A fase observada
de login também registrou 250.000 microssegundos de throttling no proxy e
zero nos grupos de apps/bancos. Essa janela de recursos inclui bootstrap
do k6 e não estabelece causalidade com uma amostra individual de latência.
Os resultados reprovados e seus denominadores permanecem públicos.
[Achados](results/stress/fleet-5-sharing-smoke-findings.json) e
[evidência completa](results/stress/fleet-5-sharing-smoke.json).

Após confirmar a limpeza e a ausência de paradas críticas, iniciou
`fleet-caddy-5-sharing-final-pass-1-001`: cinco clusters novos, etapas de
300 segundos, teto provisório de 32/s por projeto, mesmos limites absolutos,
90%/20% de interferência e drenagem de 30 segundos, conforme a pré-declaração.
O smoke não foi promovido a aprovação ou descartado pelo início dessa campanha.

Os **18 checks do CI de `93d6723` passaram**, incluindo o perfil de CPU e suas
regressões; isso não transforma a falha de serviço acima em aprovação.
[Checks e URLs](results/stress/ci-93d6723.json).

### 2026-10-16 17:00 UTC — interrupção crítica e segunda auditoria dos contadores

`fleet-caddy-5-sharing-final-pass-1-001` foi **interrompido automaticamente**
na fase noisy-login: a chamada `docker stats` do gerador excedeu 10 segundos.
Concluiu 2.790 segundos efetivos e 604 observações, sem OOM observado;
limpeza confirmada. Os degraus de capacidade 8/16/24/32 por projeto, três
balanced e recarga real do proxy concluíram. Noisy-mixed reprovou 24 gates
relativos de latência nos quatro projetos saudáveis. A conciliação final de
identidades/markers não foi alcançada; sua aprovação não é inferida dos
contadores intermediários. Não há causa de produção confirmada para o timeout.
[Evidência](results/stress/fleet-5-sharing-final-pass-1.json).

A auditoria adicional confirmou perda de séries raras também em `http_reqs`
e `expected_rejections`. As três campanhas históricas de quatro horas
registraram zero rejeições esperadas, mas cada uma contém **3.937** na
identidade independente: asserções aprovadas menos operações úteis, rejeições
de sobrecarga e verificações adicionais de corpo. A fórmula foi confrontada
com 97 relatórios curtos e não apresentou divergências. No S3, 88.528
requisições HTTP adicionais são redirecionamentos 302 reais; portanto contagem
lógica não pode substituir os contadores físicos por status/método/workload.
Os relatórios originais continuam preservados; não se recuperam contadores
perdidos de dropped iterations que não tenham observação histórica suficiente.

A correção em validação mantém séries por fase, com polling contínuo,
lookback de 120 segundos, intervalo máximo de 30 segundos e estado limitado.
Contadores físicos são conciliados com asserções independentes. O gerador
passa a oferecer leitura real de cgroup após timeout do stream Docker,
conferindo container, rótulo e identidade do processo, sem ampliar os limites
de observação. Os 235 testes Python passaram em 13,314 segundos; o controle
nativo com expiração HTTP e timeout injetado ainda estava em execução neste
checkpoint. Essas correções de medição exigem novas campanhas afetadas.

A contagem conservadora atual inclui apenas intervalos efetivos completos;
preparação, warmup, descanso, controles artificiais e intervalos incompletos
permanecem excluídos. [Checkpoint 008](results/stress/load-time-checkpoint-008.json).

### 2026-10-16 17:17 UTC — controle HTTP nativo aprovado após correção do oráculo

O primeiro controle `sparse-http-native-control-001` reprovou e foi limpo.
Seu gate exigia que a série instantânea de 200 ainda existisse depois da saída
do gerador, embora o lookback artificial de 2 segundos possa expirá-la também;
não salvou todos os valores dessa comparação, limitação preservada. O controle
foi corrigido para salvar resultados antes do gate e distinguir a expiração
rara dos 401 da expiração final do tráfego positivo.

`sparse-http-native-control-002` passou em 141,026 segundos: o servidor recebeu
273 requisições, exatamente 3 com 401 e 270 com 200. A coleta contínua reteve
273/3; as consultas instantânea e de 120 segundos perderam os 401 e a rejeição
esperada, enquanto a janela de 120 segundos conservou exatamente os 270
positivos. A instantânea geral também expirou para zero. Foram 30 observações,
140,012 segundos de cobertura e intervalo máximo de 6,496 segundos.

Um timeout de `docker stats` foi injetado deliberadamente, sem alterar o daemon
ou containers alheios: a leitura alternativa encontrou o mesmo container/PID,
7 tarefas, 12.214.272 bytes e 136.129 microssegundos de CPU, com OOM zero e
limites nativos de 0,25 CPU/128 MiB conferidos. Limpeza confirmada. O controle
não usa o backend e não acrescenta tempo efetivo ao G05.
[Antes/depois](results/stress/native-http-control-before-after.json) e
[auditoria histórica](results/stress/historical-http-counter-audit.json).

### 2026-10-16 17:33–17:36 UTC — smoke fechado e latência de redirecionamento corrigida

`disk-http-counter-smoke-001` passou: 605 segundos registrados, **590 efetivos
sem warmup**, 58 fases com cobertura fechada dos contadores, 151 observações,
um milhão de linhas por tabela, payload de 1 MiB e limpeza confirmada.
O perfil inclui calibração curta, sobrecarga e cinco picos/recuperações;
não substitui etapas de cinco minutos nem endurance de quatro horas.
[Evidência](results/stress/disk-http-counter-smoke.json).

Um controle nativo independente revelou que `response.timings.duration` do k6
mede somente a resposta final após redirecionar. Um primeiro hop atrasado em
250 ms produziu cerca de 5,5 ms no campo final contra 257 ms de duração lógica.
Isso afeta a medição de downloads S3 e também excluía DNS/conexão/espera.
A correção usa `execution.instance.currentTestRunDuration`, relógio monotônico
do k6, antes/depois da requisição completa, com validação de finitude e sinal.
O protocolo novo é obrigatório nos comparadores: dados anteriores continuam
preservados e não certificam regressão segundo esse contrato. O backend Rust
continua idêntico; não se atribui o achado à implementação de produção.
[Controle da API](results/stress/native-redirect-timing-api.json).

A regressão usa o **workload real** antes/depois, mantendo validação de bytes
e atraso controlado de 250 ms: p95 anterior **6,165 ms**, corrigido **256,798 ms**,
8 downloads redirecionados em cada versão. Os 18 casos nativos tiveram os
resultados esperados, incluindo rejeições intencionais de payloads inválidos;
limpeza confirmada. Os **237 testes Python passaram em 13,490 segundos**.
[Antes/depois](results/stress/native-logical-latency-before-after.json).
Os 18 checks do commit precedente `c276cc3` passaram; o novo commit de latência
terá CI próprio e campanhas frescas. [CI precedente](results/stress/ci-c276cc3.json).

### 2026-10-16 17:40–17:59 UTC — limite S3, retenção física e auditoria do provedor

`s3-http-latency-smoke-001` **reprovou** durante fresh-storage-3: o uso físico
ultrapassou o limite declarado de 1.024 MiB. Concluiu 95 segundos registrados,
**80 efetivos sem warmup**, com 16 observações. A última amostra persistida
registrou 999 MiB; o valor exato que acionou o guard não foi salvo. O coletor
foi corrigido para persistir valor observado, limite, componente e fase antes
de interromper futuros testes por orçamento de disco. Não houve OOM observado;
limpeza confirmada. A conciliação final não foi alcançada.
[Evidência reprovada](results/stress/s3-http-latency-smoke-failed.json).

No diagnóstico isolado do RustFS, 16 uploads de 1 MiB seguidos de DELETE
deixaram a API com zero objetos/multipart, mas os bytes continuaram na lixeira
física. A fonte associada à imagem declara coleta automática a cada 300
segundos; essa associação não constitui atestado independente do build.
O diagnóstico observou o ciclo real, **sem forçar GC**, e sua limpeza.

Os oráculos privados originais também falharam: a tentativa 002 exigia zero
KiB, ignorando blocos do diretório vazio; a 003 presumiu uma entrada física por
objeto lógico, embora o provedor mantenha dados/metadados separados e lixo
transitório de inicialização. Ambos os veredictos reprovados continuam intactos.
A auditoria corrigida foi executada separadamente sobre os dados congelados,
com regressões para diretório vazio, lixo inicial, múltiplas entradas, dados
residuais e evidência ausente/não finita. Não reescreve o veredicto do produtor.

Na 003, o início tinha 1 MiB e uma entrada transitória; após o DELETE,
17 MiB/49 entradas; ao final, **1 MiB/zero entradas**, 4 KiB do diretório vazio
e API ainda sem objetos ou multipart. A análise confirmou liberação de 16.712
KiB e 49 entradas em 330,374 segundos de observação após DELETE, sem OOM
observado. Isso explica retenção física temporária deste experimento, sem
certificar ausência absoluta de vazamentos ou atribuir todo crescimento de
memória do provedor a essa causa.
[Dados originais e análise separada](results/stress/native-s3-trash-reclamation.json).

O perfil futuro de objetos grandes S3 recebe orçamento físico de **4.096 MiB**,
pré-declarado antes da próxima execução e condicionado ao preflight/reservas.
O disco local e o endurance ordinário com objetos de 17 bytes continuam com
1.024 MiB. Payload, taxas declaradas, SLOs e integridade permanecem iguais;
a tentativa anterior de 1 GiB não é promovida. Não se altera a coleta do
provedor. [Plano atualizado](results/stress/final-campaign-plan.json).

Os **244 testes Python passaram em 13,247 segundos**, e os 18 checks de
`b82699f` passaram. [CI](results/stress/ci-b82699f.json).
O [checkpoint 010](results/stress/load-time-checkpoint-010.json) mantém somente
carga efetiva concluída; os diagnósticos artificiais de retenção não entram na
contagem de stress do BaaS. Endurance no novo protocolo ainda não foi aprovado.

### 2026-10-16 19:23–19:38 UTC — limite de compactação das métricas corrigido

`disk-final-pass-1-001` **reprovou** no início do endurance: o guard capturou
**522 MiB** no armazenamento do Prometheus, acima do limite de **512 MiB**.
Concluiu 4.305 segundos registrados/**4.290 efetivos sem warmup**, 833
observações e limpeza. A parte contínua interrompida não recebe crédito de
quatro horas; a conciliação final não foi alcançada. Os 61 relatórios completos
anteriores mantêm cobertura fechada de HTTP/arrivals, capacidade qualificada
400/s e teto pré-declarado; o mix fechado medido foi 665,5/s, sem alegação de
capacidade máxima. O endurance pretendido era 280/s.
[Evidência reprovada](results/stress/disk-final-pass-1-failed.json).

O valor de 522 MiB foi registrado pelo guard antes da interrupção; as linhas
completas de recursos registraram no máximo 10 MiB, pois a alocação foi
transitória. Não se substitui o pico do guard pela série amostrada.
[Série numérica filtrada](results/stress/timeseries/disk-final-pass-1-failed.csv.gz)
e [índice/limites](results/stress/timeseries/disk-final-pass-1-failed.csv.index.json).

A causa é a pré-alocação do writer de blocos: no Prometheus 3.5.0,
`DefaultChunkSegmentSize` é 512 MiB. O segmento inteiro é reservado antes
de ser truncado ao tamanho dos dados, independentemente da retenção de 128 MiB
e dos segmentos WAL de 16 MiB já configurados. A correção estabelece
`--storage.tsdb.max-block-chunk-segment-size=16MB` e o caminho explícito
`/prometheus`. **O guard continua em 512 MiB**, sem aumentar CPU/RAM/retenção.
O perfil comum é registrado nos dois executores/comparadores; o fleet também
passa a observar o disco das métricas em cada ciclo. Futuros estouros registram
o instante monotônico junto ao valor e ao limite.

O controle nativo 001 reprovou por assumir o caminho de dados sem explicitá-lo;
o resultado foi preservado e a limpeza confirmada. O 002 passou com o caminho
explícito. O **003 passou na versão final do gerador de argumentos**, em
3,867 segundos: a API confirmou 16 MiB, os 2.000 incrementos foram preservados
e quatro snapshots reais geraram blocos com 1.004 séries/amostras/chunks
cada. Pico **amostrado** de 1 MiB, sem OOM ou violação observada; esse valor não
é anunciado como pico físico absoluto. A API administrativa foi habilitada
somente no controle próprio em loopback, não nas campanhas ordinárias.
[Antes/depois e código de referência](results/stress/native-metrics-compaction-before-after.json).

Os **246 testes Python passaram em 13,399 segundos** na versão final do
builder (`python-checks-023.log`); a execução anterior em 13,472 segundos
também foi preservada. O controle 004 repetiu os quatro snapshots com
verificação da propriedade do listener antes da prontidão e de cada snapshot,
e também passou com limpeza confirmada. Os 18 checks de `c769934` passaram antes desta
correção; o novo commit terá CI próprio. [CI anterior](results/stress/ci-c769934.json).
O [checkpoint 011](results/stress/load-time-checkpoint-011.json) exclui a fase
contínua interrompida e todos os controles sintéticos.

### 16/10, 20:15 UTC — CI e inventário de VM nativa

Os 18 checks de `1f39159` terminaram com sucesso, incluindo o controle nativo
da compactação. A nova campanha `disk-final-pass-1-002` continua no artefato
congelado desse commit; ainda não completou endurance e não foi aprovada.

O workflow manual recebe um alvo separado `native-preflight`, executado
somente em VM descartável `ubuntu-latest` do GitHub. Esse alvo exclui o job
de benchmark em runner próprio, cujo ambiente não foi autorizado para uma
carga adicional. O inventário é somente leitura, com prazo de cinco minutos:
CPU, RAM/disco disponíveis, systemd, sudo e KVM. Não instala pacotes, não
inicia serviços, não acessa dados de projetos e não representa aceitação
nativa nem perda de energia. A disponibilidade efetiva será registrada após
o dispatch; não é presumida a partir do arquivo do workflow.

O [inventário real](results/stress/native-vm-capacity-001.json) passou na
execução `38960782906`: quatro CPUs, 16.766.406.656 bytes de RAM total,
92.078.571.520 bytes livres em disco, PID 1 systemd, sudo disponível e
`/dev/kvm` presente. O usuário comum não tinha permissão de leitura/escrita
nesse dispositivo; a disponibilidade para o controlador privilegiado ainda
depende de sua verificação efetiva. O job em runner próprio foi ignorado.

Foi preparado um alvo manual **`native-vm`**, separado dos benchmarks locais:
Ubuntu 24.04 descartável com convidada KVM, disco virtual de 12 GiB, duas
vCPUs, 3 GiB de RAM e watchdog independente. O protocolo mantém validação
de TLS e da chave SSH. Pretende verificar systemd, RLS, arquivos privados,
backup/restauração e perda da RAM da convidada, comparando confirmações
registradas pelo observador externo. Os seis testes rápidos dos limites de
segurança passaram em 0,005 segundo, sem criar serviços ou máquinas locais.
**A campanha nativa ainda não foi executada** e não recebe aprovação antecipada.
Restauração em alvo novo, fora do host, PITR, perda física de energia e ACME
público continuam separados dessa verificação funcional.

### 16/10, 20:34–20:40 UTC — primeira VM real, falha do oráculo e limite de diagnóstico

A [primeira execução nativa](https://github.com/Nelcota/Nelcota/actions/runs/38962480993)
conseguiu usar KVM, iniciar uma Ubuntu 24.04 nova, instalar o runtime nativo,
iniciar app/PostgreSQL/Caddy, reaplicar a migração da aplicação e verificar
os limites reais de CPU/memória/swap/PIDs no kernel. HTTPS com a CA local
validada passou, a CA não confiável foi rejeitada e o segundo projeto nativo
foi recusado. O binário compilado nessa máquina tem SHA-256
`f79fc282439f9f496b720953965b88bd99e195c695e9c6411b131821d665d0b3`;
esse artefato é distinto do binário local e não recebe seus números de capacidade.

**Resultado completo: reprovado.** O primeiro signup retornou o `201`
documentado, mas o executor novo ainda esperava o `200` genérico. O oráculo
foi corrigido para exigir exatamente `201`, token não vazio e identidade
correta; não se alterou o contrato da API. Backup/restore, perda de RAM e
reconciliação externa **não foram alcançados** nessa tentativa.

A mensagem de resultado da anotação do Actions foi truncada exatamente em
4.096 caracteres. O download do artifact voltou a retornar HTTP 403; o
recibo final de limpeza ficou fora da parte acessível, portanto não é
presumido. A execução descartável terminou, mas isso é diferente de ler o
recibo específico do QEMU. O publicador agora divide o JSON público em
fragmentos limitados com SHA-256 e índices; regressão reconstitui sem perda
um relatório maior que o limite, inclusive escapes, Unicode e `%`.
O workflow pode reler o artifact público anterior dentro do próprio GitHub,
preservando seu veredito original, antes de repetir a campanha.

Onze testes focados passaram em 0,006 s. A primeira execução dos novos
testes falhou em quatro casos porque o mock tratava `assert_safe` como
assertion inexistente; a fixture foi corrigida e a saída posterior foi
preservada. Na próxima VM o slice agregado recebe teto de **1,5 CPU**,
reservando **0,5 CPU** para o SO da convidada, com verificação efetiva no
kernel. Quotas individuais somam 2,25 CPUs dentro desse teto explícito;
memória, disco, tráfego, TLS, duração e demais guardas não foram ampliados.

### 16/10, 20:45–20:50 UTC — job nativo passou; segunda limitação das anotações

A [repetição nativa 002](https://github.com/Nelcota/Nelcota/actions/runs/38963816198)
terminou com sucesso. O resumo íntegro confirma 140,169 s incluindo preparo,
27 amostras, `cleanup_confirmed=true` e SHA-256 do relatório
`68a282ed4b290b365524a0236eecf4b2e226ad5e5a95eaeb0066978265fdd5da`.
O código congelado é `7e1bd25`; o relatório detalhado ainda precisa ser
recuperado antes de publicar suas medidas de recovery e reconciliação.

Os sete fragmentos da **tentativa 001 reprovada** foram recuperados e
reconstituídos com SHA-256 verificado, preservando o veredito original.
Seu [relatório completo](results/stress/native-vm-runtime-first-failure.json)
confirma limpeza, 13 amostras e 65,145 s incluindo preparo. Nenhum tempo de
endurance é acrescentado por essa tentativa.

Na própria tentativa 002 surgiu outro limite real do Actions: além do
máximo de bytes, só dez anotações foram retidas no step. Resumo mais dez
fragmentos suprimiu o último fragmento; o reconstituinte recusou a evidência
incompleta. O transporte passa a usar gzip/base64, SHA-256 do JSON original
e **no máximo nove fragmentos mais resumo**. Doze testes focados passaram
em 0,009 s, cobrindo também o orçamento de quantidade e a rejeição de
relatório grande demais. A próxima execução relerá o artifact original
da 002 dentro do GitHub.

Foi preparado um caso separado `cluster-routing`, ainda **não executado**:
dois clusters PostgreSQL fictícios dentro de uma VM nova, `17/main` na 5433
e `17/sentinel` na 5432. Testará se `init` rejeita o layout incompatível
antes de alterar a senha do cluster alheio ou criar configuração. Nenhum
defeito de roteamento de produção é declarado sem essa reprodução. O
cluster extra tem CPU/RAM/PIDs próprios dentro do mesmo slice agregado,
e o limite físico de PostgreSQL soma os dois diretórios. Os 18 checks
ordinários de `e80770b` passaram; o job manual nativo reprovado permanece
registrado separadamente, sem anunciar o conjunto inteiro como aprovado.

### 16/10, 20:59–21:10 UTC — defeito real de seleção de cluster reproduzido

O [caso nativo 003](https://github.com/Nelcota/Nelcota/actions/runs/38965444222)
**reproduziu um defeito de produção**, usando somente dois clusters criados
para o teste dentro da VM. Com `17/main` na porta 5433 e `17/sentinel` na
5432, `init` retornou exit 0, criou configuração e alterou a senha do
`postgres` no **sentinel**, que não era o cluster nomeado pelo runtime.
Presença de senha passou de false para true; a linha sentinela continuou
com count 1. As consultas antes/depois qualificaram explicitamente cada
cluster. Não se publicaram senha nem hash de credencial.
[Reprodução completa](results/stress/native-postgres-collision-before.json):
88,188 s incluindo preparo, 17 amostras, nenhum guard crítico acionado e
limpeza confirmada. Ambos os clusters estavam em cgroups próprios com os
limites declarados; não foi usado PostgreSQL do ambiente compartilhado.

A causa é a divergência entre a identidade fixa `17/main`/5432 e o
`runuser ... psql` sem destino explícito. O wrapper Debian escolheu o
cluster que atendia na porta padrão. A correção candidata valida o layout
no preflight, antes de instalar pacotes ou escrever o host, e novamente
antes de configurar o cluster. Clientes nativos passam a selecionar
explicitamente o cluster/versão e socket/porta/usuário/banco locais,
limpando overrides de host/service herdados. O password gerado sai dos
argumentos de processo e é enviado por stdin, sem eco do SQL de erro,
com prazos de conexão/statement/lock. Foram adicionadas seis regressões
Rust para layouts válidos/incompatíveis/ambíguos, NaN/portas inválidas,
roteamento herdado e limites da operação de senha. **Ainda não executadas
neste checkpoint**; compilação, testes e repetição nativa são a próxima
validação, em máquinas separadas para preservar o soak local.

O mesmo workflow recuperou integralmente o relatório da campanha nativa
normal 002, em dois fragmentos comprimidos, mantendo o SHA-256 original.
[Resultado completo](results/stress/native-vm-runtime-pass-1.json): 30 ACKs
externos reconciliados após perda da RAM da VM, zero perdas/duplicações/
extras, boot de kernel diferente, bytes privados preservados, restore
local de banco/arquivos em **1,606 s** e recuperação da convidada em
**13,815 s**. `remove` eliminou unit/cluster e o QEMU foi parado com recibo.
Não houve mutação em andamento no instante da interrupção; restore foi
no mesmo alvo, sem alegar perda de energia do host físico, PITR ou off-host.

O soak de `disk-final-pass-1-002` começou por volta de **21:01 UTC** e
continua sem reiniciar app/banco. A mudança real na CLI exigirá um novo
binário: essa execução preserva seu hash anterior como **baseline**,
sem ser promovida silenciosamente a evidência do artefato corrigido.
As campanhas afetadas terão novas medições depois da compilação/verificação.

### 16/10, 21:20 UTC — bloqueios de CI da correção candidata preservados

O CI de `0028307` reprovou nos steps de **Clippy** e **`npm run check`**.
Os testes Rust ainda não foram anunciados como aprovados. O módulo de testes
nativo foi movido para depois do helper de remoção, conforme a regra de
ordenação do Clippy; a mensagem exata do job ainda será recuperada pelo
diagnóstico remoto, sem inventar a causa a partir do exit 101 genérico.

O checker de guias confirmou uma tradução ausente no novo parágrafo de
`docs/deploy.md`. O catálogo português foi atualizado e o check estático
passou, preservando inline code e links. O bundle versionado ainda precisa
ser reconstruído em commit próprio durante um intervalo seguro entre
campanhas; não se iniciou build/teste pesado local junto ao soak.

O inventário manual pode agora recuperar apenas cabeçalhos públicos de
diagnósticos dos jobs falhos de lint/painel, com token somente de leitura,
prazo e limite de arquivo. O download local dos logs continua indisponível;
logs completos, stderr de URLs assinadas e dados de runtime não são publicados.
O caso de colisão também passa a exigir a mensagem específica do guard de
layout e a verificar o SHA-256 do binário dentro da convidada, evitando
aprovar uma rejeição genérica ou um executável diferente. A identidade do
artifact anterior será publicada separadamente sem mudar seu relatório.

### 16/10, 21:31–21:48 UTC — correção nativa comprovada e fluxo normal revalidado

A [repetição da colisão](results/stress/native-postgres-collision-after.json)
passou na execução `38969205271`, com binário
`fff01664c28916ddb67eb8d4f9852c5b443df710806102490fcace27fff3ba31`.
`init` retornou exit 1 com a mensagem específica do guard de layout; senha
sentinela inalterada, count 1, nenhum host ou arquivo de configuração nativa
criado. Onze amostras, 55,099 s incluindo preparo, sem violação crítica e
limpeza confirmada. A [identidade original do baseline](results/stress/native-postgres-collision-before-artifact.json)
foi recuperada separadamente: binário `b424257b6c82b7e9051d29897ee3eedc7c2460355db88e248fbfa119a6065b02`.
Não se presumiu igualdade com outros builds de código semelhante.

O [fluxo normal candidato](results/stress/native-vm-runtime-candidate-pass-1.json)
também passou na execução `38970590208`, código `6f8a3e7`, binário
`0cccc84840b5d09ee76b1ab2005b52e7d702bd44c9f9c7662df5c6bde82e110f`.
Incluiu senha por stdin, RLS, bytes privados, backup/restore e perda da RAM
da convidada. Trinta ACKs externos preservados, zero perdas/duplicações/
extras; restore no mesmo alvo **2,108 s**, recuperação da convidada
**16,367 s**. São medidas funcionais desse build/VM, sem afirmar regressão
ou melhoria de performance em relação ao build anterior. Trinta e duas
amostras, 168,203 s com preparo, sem guard crítico e limpeza confirmada.
O binário efetivamente usado foi retido como artifact de CI por 14 dias;
isso não publica uma release.

No CI de `dec797d`, Clippy e o job Rust/aceitação completo passaram. O job
do painel passou pelos checks/testes/build e reprovou no guard esperado de
`dist` versionado, ainda pendente do rebuild separado no intervalo seguro.
O primeiro diagnóstico remoto dos logs falhou e seu workflow foi preservado
(`38969200170`); a versão seguinte também publica recibo limitado da própria
falha de download, sem URLs assinadas ou stderr bruto.

Foram preparados dois probes adicionais de instalação, ainda sem veredito:
diretório atual padrão do guia e tentativa de segundo host nativo em outra
pasta na mesma VM. Nenhum novo defeito é declarado antes da reprodução.
As próximas campanhas nativas passam a registrar versões efetivas de
PostgreSQL/client/Caddy/systemd, kernel/libc e Python. Doze testes rápidos
do executor passaram em 0,011 s; nenhum build ou suíte pesada local foi
iniciado junto ao soak em andamento.

### 16/10, 22:06 UTC — checkpoint técnico e pendência CodeQL explícita

O relatório português e a matriz foram atualizados com os resultados nativos
já concluídos, sem promover linhas inteiras nem o G05. A repetição do fluxo
nativo corrigido preservou seus 30 ACKs, bytes e identidades após reiniciar
a convidada. A colisão PostgreSQL tem reprodução antes/depois e o fluxo
normal corrigido também passou. O tempo efetivo consolidado continua
100.044 s; a campanha aberta não foi somada parcialmente.

Os probes de diretório padrão (`38973154487`) e segundo host em outra pasta
(`38973642284`) usam VMs novas do Actions, serializadas no mesmo grupo de
concorrência. O segundo ainda estava na fila; nenhum desses cenários foi
anunciado como aprovado ou como defeito confirmado.

O CI de `6f8a3e7` passou em Rust/Clippy, upgrade, SDKs, lint e supply chain.
O painel reprovou somente no guard `versioned dist is up to date`; o build
separado será feito no intervalo local seguro. O check agregado do CodeQL
**também reprovou**, apesar dos três jobs de análise concluídos com sucesso:
a anotação `Hard-coded cryptographic value` apontou a lista de senhas inválidas
no teste de `native/postgres.rs`. O endpoint de alertas continua HTTP 403,
mas a anotação do check forneceu arquivo, linha e mensagem.

A fixture foi alterada para gerar uma base aleatória com o mesmo gerador
seguro do projeto e acrescentar cada caractere inválido; o teste agora exige
o erro específico do validador, evitando aceitar uma falha posterior de
provisionamento como sucesso da regressão. Vazio/tamanho continuam cobertos
no validador. A regra de segurança não foi desligada nem o alerta dispensado.
Rustfmt passou após aplicar a quebra de linha exigida; testes Rust e novo
CodeQL ainda dependem do CI, sem build pesado local junto ao soak.

O segundo diagnóstico remoto limitado (`38973148545`) reprovou com exit 1
sem código HTTP extraível. O recibo foi preservado, sem concluir uma causa
que a evidência não identifica e sem publicar URLs assinadas ou stderr bruto.
No monitor local às 22:05:45: soak ativo há aproximadamente 65 min,
1.423 amostras, idade de 1,1 s, app RSS 46.320 KiB, banco 320 MiB,
zero erros de assinatura e zero OOMs observados.

### 16/10, 22:12 UTC — instalação documentada reproduz falha de caminho relativo

O probe `default-root` terminou **reprovado**, com limpeza confirmada e sem
violação de segurança do staging: [relatório original](results/stress/native-default-root-before.json),
run `38973154487`, código `77d2335`, binário
`e5552ea9fa39b1a64491ba1d61da652c671e1375e565be3087dcdf8e93068533`.
Rodar `init` no diretório da aplicação, sem `-C`/`NELCOTA_ROOT`, retornou
exit 0, mas o `EnvironmentFile` da unit era relativo. `up` excedeu o prazo
de 30 s; a unit estava sem PID ativo, em auto-restart, com 14 reinícios.
`systemd-analyze verify` isoladamente retornou 0, portanto não substitui
ativação/readiness real nesse caso. As versões efetivas constam no relatório.

A CLI preservava `.` em `Host::root()` e escrevia diretamente esse caminho
no arquivo de ambiente da unit. A correção resolve `cli.dir` para absoluto
antes de construir o host e antes das operações de deploy/lease. Isso mantém
o diretório escolhido, sem depender do cwd de systemd. O probe real existente
é a regressão de ponta a ponta: exige caminho absoluto, ativação e HTTPS
válido, e será repetido com novo binário/VM. Rustfmt e diff-check passaram;
nenhum teste nativo foi executado contra a máquina local compartilhada.

### 16/10, 22:31 UTC — diretório padrão aprovado; colisão de hosts reproduzida

A [repetição do diretório atual](results/stress/native-default-root-after.json)
passou em `38974387655`, fonte `b43dc58`, binário
`5d8fe465c129a68c5ad334d340211c1a099a322b83adfff52f86db7d1f792885`:
`EnvironmentFile` absoluto, init/up/verify exit 0, HTTPS validado e limpeza
confirmada. Dezenove amostras, 98,166 s incluindo preparação. Esse protocolo
verificou ativação; não certificou todas as operações da CLI executadas a
partir do cwd do host.

O [probe de outra pasta](results/stress/native-host-root-before.json) reprovou
em `38973642284`, fonte `77d2335`, binário
`b46e5801ff2f545090cfca27dfae02b1c819d4921458c6980044def4d8615bfa`.
O segundo init retornou 0, mudou a senha do primeiro PostgreSQL e seu Caddy,
e criou outro manifesto e outra unit. A linha sentinela permaneceu; isso
não torna a instalação segura nem prova que novas conexões antigas funcionam.
Tudo estava em uma convidada descartável, com limpeza confirmada.

A correção adiciona lease global e `owner.json` privado/root-owned em
`/var/lib/nelcota-native`. A propriedade é persistida antes dos pacotes e
configuração nativa, fica reservada após erro/interrupção e só é liberada
após **todas** as etapas de remoção concluídas. O inode do lock permanece.
Outro diretório, aliases ambíguos, registros corrompidos/grandes e symlinks
não autorizam tomar a instalação. Reconhecimento de instalação anterior
exige unit compatível e caminho absoluto do `.env` do próprio host.

A revisão também encontrou uma janela na primeira criação: a seleção da
lease anterior dependia de o manifesto já existir. Inicializações novas
agora tomam a lease comum antes de publicar o primeiro manifesto; init existente
relê o registro sob a lease e rejeita mudança concorrente. Operações do host
validam a propriedade nativa enquanto mantêm a lease do diretório. Migrate
congela a escolha entre host e configuração direta, mantendo sua lease pelo
trabalho, sem transformar um host recém-publicado em alvo sem proteção.
O teste de corrida usa locks reais e uma barreira enquanto o primeiro owner
ainda não foi publicado; outros testes cobrem owner durável, troca/corrupção,
limpeza completa, symlinks, aliases e adoção legada. Os novos testes Rust ainda
aguardam compilação/execução no CI. Nenhuma reprodução de corrida concorrente
real na CLI inteira foi afirmada a partir apenas desses testes de primitivas.

O CodeQL de `b43dc58` reprovou novamente, agora com duas anotações de fixture
([recibo](results/stress/codeql-invalid-fixtures-second-failure.json)). A consulta
pública `HardcodedCryptographicValueExtensions.qll` identifica argumentos por
nome do parâmetro (`password`), inclusive em validadores puros; também possui
barreira de concatenação de dado não constante. O validador passa a chamar sua
entrada de `candidate`, e o teste concatena uma base aleatória com o caractere
inválido. As funções que de fato configuram/alteram senha mantêm seu parâmetro
de credencial e o CodeQL segue habilitado. Isso preserva exatamente os inputs
malformados e a rejeição específica; novo resultado está pendente.

Doze regressões Python do executor passaram em **0,012 s**, rustfmt,
fronteiras/checksums de migrations, diff-check e catálogo português passaram.
Build de painel/binário e suítes pesadas locais continuam adiados até terminar
o soak congelado, que não teve sua continuidade interrompida por essas alterações.

### 16/10, 22:35 UTC — repetição de ownership e extensão da verificação de cwd

A correção de ownership está em `f560493`; o probe `host-root` foi disparado
para uma nova VM com esse código. O runner exige a mensagem específica de
rejeição, senha/Caddy/linha inalterados e HTTPS válido da primeira instalação.
O diretório rejeitado pode conter metadados de lease privados; isso não é
criação de outro manifesto/projeto/unit, e será mostrado separadamente.

A revisão do caminho de migração identificou outra hipótese: um subprocesso
nativo pode herdar o cwd/NELCOTA_ROOT do host e tentar retomar a lease já
mantida pelo pai. O probe `default-root` foi ampliado para executar migrate
real nas duas configurações, verificar a tabela e uma única aplicação da
migration. O código da aplicação ainda não foi alterado para essa hipótese;
o resultado da nova VM determinará se existe falha real. O resultado aprovado
anterior continua válido somente para seu protocolo de ativação/HTTPS.

### 16/10, 22:38 UTC — revisão da liberação da propriedade nativa

A revisão do código recém-adicionado encontrou um caso de lifecycle que
precisava de proteção: após remove liberar o owner, `up` no host vazio antigo
não pode criar outra reivindicação nem alterar recursos globais. A lease
básica do diretório foi separada da validação de mutação nativa; init continua
permitido em um host vazio, enquanto os demais comandos exigem um único
projeto nativo antes de reivindicar/verificar a máquina. A regressão usa um
manifesto vazio próprio e verifica a rejeição específica e a lease de init.

O runner nativo passa a executar esse `up` após remove e confirmar que o
owner segue ausente. Esse refinamento veio da revisão do fluxo, sem alegar
uma falha observada numa campanha inteira já executada. A nova regressão
Rust/VM ainda está pendente. Rustfmt, fronteiras/checksums e diff-check passaram;
o soak local continua no mesmo binário e processos.

### 16/10, 22:45 UTC — migração nativa no cwd reprovada; ownership e CodeQL aprovados

A [nova verificação de migrações](results/stress/native-cwd-migration-before.json)
reprovou na VM `38976562183`, fonte `f3c8d49`, binário
`e81f861e7a5f4b4c9979a3be678e1a7a7b42cd4aab01daf734029293f12ca7be`.
Init/up/verify passaram e o arquivo de ambiente era absoluto, mas **as duas
migrações retornaram exit 1 com `deployment_busy`**. Nenhuma tabela de fixture
foi criada. Catorze amostras, 70,139 s com preparo, limpeza confirmada.

A CLI nativa criava seu subprocesso com a configuração do projeto, mas sem
selecionar seu diretório. Ele herdava o cwd/NELCOTA_ROOT do host e tentava
a mesma lease já mantida pelo pai. O subprocesso agora recebe explicitamente
`--dir <projeto>`; a configuração direta de banco continua sob a operação
do pai. O teste real exige ambos os modos, tabela criada e uma única
migration registrada. Nova VM precisa confirmar o resultado; não se inferiu
sucesso apenas da alteração de argumentos. Rustfmt/fronteiras/diff-check passaram.

O [probe de propriedade](results/stress/native-host-root-after.json) passou
em `38976428540`, fonte `f560493`, binário
`27a7fb004db277ec59bd071f0cba8f009e31f416d710cda7880daa3bc5c844ae`:
segunda pasta recusada com a mensagem específica, senha/Caddy/linha originais
inalterados, nenhum segundo manifesto/unit, HTTPS válido e limpeza. Catorze
amostras, 72,177 s incluindo preparo. O [CodeQL](results/stress/codeql-invalid-fixtures-before-after.json)
agregado e a análise Rust desse SHA também passaram. O job Rust de regressões
completo ainda estava em andamento; não se anunciou a matriz inteira aprovada.

### 16/10, 22:56 UTC — errata do census SQL e repetição preservada

A revisão do probe novo encontrou um erro de tipo no oráculo: `sql()` retorna
bytes, mas a presença da tabela era comparada com a string `"t"`. Portanto
o campo `fixture_table_created: false` da execução `38976562183` não é uma
prova independente da ausência da tabela. Os dois exits 1 e a presença real
de `deployment_busy` continuam evidência independente da falha do subprocesso.
A afirmação anterior de ausência verificada da tabela fica retirada.

O census agora exige booleans PostgreSQL em bytes (`b"t"`/`b"f"`), registra a
presença do journal separadamente e valida sua contagem antes de aceitá-la.
Três regressões cobrem bytes reais, ausência sem contagem inventada, respostas
malformadas/NaN e contagens inválidas. **Quinze testes focados passaram em
0,014 s**; a duração exata no log privado prevalece sobre este valor arredondado.
A VM `38978164074` já estava executando o snapshot anterior; será preservada
com seu próprio veredito, sem aplicar alteração ao artefato em execução nem
promover o census defeituoso. Uma nova repetição corrigida é necessária.

### 16/10, 23:03 UTC — controle de relógio isolado preparado

O G05 também exige testar desvio de relógio. Foi adicionado o cenário
`clock-drift` à VM própria, sem tocar no relógio do host/controlador. Ele
mantém TLS/SSH, recursos e watchdogs monotônicos; verifica acesso/RLS antes
de avançar a convidada até expirar um JWT fresco, limitado a 1.800 s.
O access deve receber 401, enquanto a credencial de serviço mais longa
continua lendo a mesma linha e acesso anônimo segue negado. A restauração
do relógio/NTP original ocorre em finally inclusive após erro do oráculo;
a prova exige offset final de até 5 s e acesso ordinário recuperado.

O manifesto registra os limites antes da operação, inclusive tolerância de
verificação de 5 s e timeout de 15 s por comando de restauração. O relatório
retém o passo realmente observado, códigos de restauração e o estado anterior
do NTP, sem tokens/claims privados. Isso é um controle funcional, sem crédito
de endurance, proteção contra um host malicioso ou alteração do relógio real.
Dois testes novos provam restauração após falha e rejeição de NaN/bool/TTL
inadequado antes de qualquer comando na convidada. Com os quinze testes do
runner/census, **17 passaram em 0,017 s**. A execução real desse cenário ainda
está pendente e não foi anunciada como aprovada.

### 16/10, 23:28 UTC — migração confirmada, relógio validado e falha Rust preservada

A [repetição com census corrigido](results/stress/native-cwd-migration-after.json)
passou em `38979033688`, fonte `3b56b25`, binário
`18769a1f2fae5abee81be42f8de4997e9e700a431eb4bc835630cccd994cd328`.
Os dois comandos migrate retornaram 0, a tabela e o journal foram observados
em SQL e a contagem foi **1**. Treze amostras, 67,099 s com preparo e limpeza.
O [snapshot intermediário](results/stress/native-cwd-migration-census-failure.json)
permanece reprovado: comandos já retornavam 0, mas o oráculo ainda comparava
bytes com string e não comprovou presença/contagem. Não foi promovido a passe.

O [controle real de relógio](results/stress/native-clock-drift-pass-1.json)
também passou, em `38980126695`, fonte `3b56b25`, binário
`1b8b0c388dd9717a69e30b344c29b14c8050d2090c4326fb2fbe12093d9e2356`.
Passo planejado de 1.020 s, observado de **1.018,326 s**; access expirado 401,
serviço autorizado preservado e anônimo negado. Relógio/NTP restaurados,
offset final **0,471 s**, acesso ordinário recuperado. A remoção real também
confirmou owner ausente e recusa de `up` no host antigo vazio. Treze amostras,
66,136 s incluindo preparo, sem guard crítico e com limpeza.

O [CI de `14725d6`](results/stress/ci-native-executable-identity-failure.json)
reprovou em `updates::runtime::tests::running_native_identity_rejects_replacement_even_with_the_same_hash`.
O Rust de `3b56b25`, com o mesmo código Rust relevante, passou, mas isso não
apaga a falha nem demonstra sua causa. A fixture copia/inicia um executável
temporário; precisam ser investigados o ponto exato e os possíveis races de
execução/observação, sem enfraquecer a validação de identidade de produção.
O painel ainda requer o rebuild separado de assets no intervalo seguro.

Foi preparada uma descoberta de telemetria do provedor em **outra VM**,
com o digest exato do RustFS histórico, controller de 0,5 CPU/256 MiB e
provider de 0,5 CPU/1 GiB, ambos com 128 tarefas. Há guardas de disco/rede/
reservas, watchdog de 300 s e limite externo de 360 s. O receptor exige nonce
privado e IP do container, decodifica somente métricas numéricas limitadas e
não publica labels privados. Ausência de export fica bloqueada; gauges/sums
não provam liberação de owners ou explicam retroativamente o plateau de 415 MiB.
São 90 s de descoberta após preparo, sem tráfego BaaS/crédito de endurance.
Seis regressões do decoder passaram em 0,026 s; quatro de receiver/transporte
passaram em 1,012 s. A descoberta real ainda não foi executada.

O monitor local passou para a célula 993. Ele só executa o census agregado
quando a própria campanha entrar no descanso final de 180 s, mantendo a
verificação de run/label/phase e recusando qualquer query durante o soak.

### 16/10, 23:38 UTC — descoberta real do alocador e dois defeitos no controle

A [primeira descoberta](results/stress/provider-memory-discovery-first-failure.json)
foi executada em `38981893129`, fonte `5fd78f2`. Foram 18 amostras e 17
exports OTLP autenticados em 102,157 s com preparo, sem carga BaaS e com
limpeza confirmada. O teste não dá crédito de endurance. O job ficou vermelho:
o uploader não podia ler o JSON 0600 escrito pelo controller root. O relatório
original foi recuperado por notice gzip, com SHA-256 validado e sem alterar
seu veredito bloqueado.

Também havia erro no oráculo: ele reconhecia `rustfs_allocator_`, mas o
provider exporta `rustfs_memory_allocator_`. O snapshot contém gauges reais
de committed/reserved/page-committed, mas
`malloc_requested_bytes_available = 0`. O contador solicitado disponível no
mesmo export **não** comprova bytes vivos nem ausência de vazamento. A correção
separa capacidade de exportação, disponibilidade de requested bytes e liberação
de owners, que continua não verificada. Uma repetição é necessária; o resultado
original não foi promovido retroativamente.

A publicação agora valida o esquema antes de criar a pasta pública e torna
somente esse JSON legível pelo uploader (0644); a pasta privada segue 0700.
Duas regressões novas verificam o prefixo real/flag de disponibilidade e a
fronteira de permissão/publicação. As seis regressões do receiver/publicador
passaram em 1,012 s, sem tráfego contra o benchmark local em endurance.

### 16/10, 23:39 UTC — diagnóstico CI limitado, sem divulgar assertions

O downloader de logs via `gh` falhou nas duas tentativas anteriores sem
código HTTP recuperável; isso ainda não identifica o motivo do teste Rust.
O novo controle usa a API HTTPS diretamente, limita 8 MiB/45 s, restringe
redirecionamentos GitHub/Azure e nunca encaminha o token API para o download
assinado. Para a etapa Rust só publica nome/local do panic e categorias fixas;
assertions, comandos, URLs e corpo de erros permanecem privados.
Cinco regressões passaram em 0,002 s. Uma primeira execução das regressões
falhou pela expectativa textual `byte budget` versus `file budget`; a asserção
foi corrigida, preservando o limite de 8 MiB. O teste de identidade original
segue reprovado até recuperar o diagnóstico e reproduzir/corrigir sua causa.

### 16/10, 23:44 UTC — categoria original recuperada e prontidão da fixture

O [diagnóstico remoto limitado](results/stress/ci-native-executable-panic-location.json)
passou: o job original `117231644395`, fonte `14725d6`, tem panic na linha
**863:64**, primeira validação, categoria `running_executable_identity_mismatch`.
Não foi falha no `spawn`, no hash ou no teste posterior de troca do arquivo.
Isso localiza a corrida de observação imediatamente após o aviso de exec do
spawn. O teste agora copia `/bin/cat` e exige um round-trip real de seis bytes
por UnixStream com leitura/escrita limitadas a cinco segundos antes de
consultar /proc. O processo próprio continua sendo terminado e recolhido em
Drop inclusive em panic. A checagem de segurança de produção não foi alterada
nem recebeu retries; a substituição pelo mesmo hash continua obrigatoriamente
rejeitada. Rustfmt passou. A execução Rust corrigida ainda precisa passar.

Foi preparado um job em outra VM para **1.000 repetições reais** do único
teste compilado, com duas CPUs/4 GiB/128 tarefas, sem swap, timeout por teste
de dez segundos, duração interna de 240 s e limite externo de 300 s. Ele
recusa zero testes, para na primeira falha, retém apenas local/categoria do
panic e não atribui crédito de carga. Três regressões do controle passaram
em 0,002 s; elas não substituem as mil repetições reais ainda pendentes.

A [descoberta corrigida do provider](results/stress/provider-memory-discovery-after.json)
passou em `38982447143`, fonte `4ffad18`: 18 amostras, 17 exports, 18 pontos
numéricos de alocador, três gauges de memória reais, publicação/limpeza
confirmadas, 96,148 s incluindo preparo. `requested_bytes_available` continua
**false**; `live_owner_release` continua **not verified**. Isso aprova apenas
a capacidade de observar o provider, sem promover os 415 MiB históricos a
cache ou ausência de vazamento. A primeira falha permanece inalterada.

### 16/10, 23:47 UTC — probe adicional de fronteira de remoção

A revisão encontrou `let _ = remove_file(PGBACKREST_CONF)` na remoção
nativa, que descarta qualquer erro. Ainda é uma hipótese operacional a
reproduzir na VM: uma configuração ownada com tipo incorreto deve causar
rejeição específica antes de unit/cluster/manifest/owner serem alterados.
O cenário remove-boundary injeta somente um diretório marcado no caminho
da configuração dentro da convidada, usa o sentinel gerado sem backup
intencionalmente e restaura a configuração em finally. O watchdog externo
continua eliminando a VM inteira no limite; não se alterou produção.
Duas regressões novas provam restauração após erro e recusa de uma pasta
preexistente; com os demais testes nativos, 17 passaram em 0,017 s.
A execução real e qualquer correção de produção seguem pendentes.

### 16/10, 23:48 UTC — mil repetições reais do teste de identidade

O [job isolado](results/stress/ci-native-executable-identity-after.json), fonte
`c98aaaf`, passou **1.000/1.000** execuções do único teste, em **9,064 s**
de execução efetiva de regressão. SHA-256 do executável de testes:
`151c748462ab3f6be46d4928f1627cdddd39283e8e39aec359d3ce49fa6ca137`.
O controller confirmou os limites reais de kernel, e cada execução comprovou
que rodou exatamente um teste e publicou sua conclusão. Nenhum output de
assertion foi exposto. Isso valida a nova prontidão da fixture e a rejeição de
substituição por mesmo hash; não substitui CI completo, não apaga o panic
original e não soma nove segundos ao crédito de tráfego BaaS/endurance.

### 16/10, 23:55 UTC — quinto defeito nativo reproduzido, correção preparada

A [VM anterior](results/stress/native-remove-boundary-before.json), fonte
`a2a0123`, binário
`e58a37f7107ddbe1cdfe2189100f65699f9711a8cfbab122e0aaa1dced31eb6b`,
reprovou o cenário: remove retornou **0**, apagou unit/cluster e alterou o
manifesto, liberando o owner apesar da configuração de backup não removida.
A exclusão dos dados do sentinel era intencional pelo --no-backup do probe;
o defeito é sucesso indevido e liberação do owner com limpeza incompleta,
não uma perda de ACKs de escrita alegadamente não solicitada. Foram 15
amostras, 78,127 s incluindo preparo e limpeza confirmada. A injeção própria
foi removida em finally, e o resultado original continua reprovado.

A correção verifica o tipo da configuração antes de destruir unit/cluster e
propaga erros posteriores de unlink. Arquivo ausente permanece compatível;
symlink é removido sem seguir/apagar seu alvo. A desativação do PITR tinha
o mesmo erro ignorado: passa a validar antes de alterar parâmetros do banco,
propagar a falha de remoção e preservar o marcador local se o unlink falhar.
O PITR real fora do host continua bloqueado; não se certifica esse fluxo por
uma prova de helper. Quatro regressões Rust cobrem rejeição antes de teardown,
arquivo ausente/presente, falha de teardown/alteração tardia e symlinks.
Rustfmt passou; essas regressões e a VM corrigida ainda precisam executar.

### 17/10, 00:06 UTC — suite nativa com artefato comum preparada

O target manual native-suite congela um ELF e seu manifesto de build antes
de distribuir o mesmo arquivo a doze casos serializados: seis cenários,
duas passagens, cada uma em runner/convidada descartáveis próprios. Nenhum
caso recompila o candidato. Source, tamanho e SHA do download são verificados
antes da VM, e identidades preservam run/attempt reais mais pass/case. Cada
job publica seus notices separadamente, respeitando o limite de dez por step.
Os casos de app também comprovam inode e bytes de /proc/PID/exe em relação
ao executável implantado/fornecido, inclusive após o power cut. O freeze
recusa overrides de compilação não declarados. São cinco regressões novas
(0,003 s), mais 17 do runner (0,013 s), todas aprovadas sem carga local.
A suite real deve ser disparada no source final após o build separado dos
assets; sua preparação não representa doze casos já executados. Os dois
artefatos finais, local de performance e nativo construído no runner, terão
hashes e escopos próprios, sem alegação de builds bit-a-bit reproduzíveis.

### 17/10, 00:07 UTC — fronteira de remoção corrigida passou em VM nova

A [repetição corrigida](results/stress/native-remove-boundary-after.json),
fonte `85b3c5a`, binário
`40791c78912c680ca8994a0f803ff3c246dcf48c74aa71d3fbe4e75404496818`,
passou: remove exit **1**, rejeição específica, owner/manifest/cluster
preservados e sentinel COUNT **1**. A configuração de tipo incorreto
permaneceu intacta durante a recusa e foi desfeita pelo finally do probe.
Em seguida, a remoção normal com backup passou e confirmou liberação do owner
somente ao final, além de recusa de up no host antigo vazio. Foram 16 amostras,
80,168 s incluindo preparo e limpeza, sem aprovação de endurance/throughput.
As quatro regressões Rust novas aguardam conclusão do CI de seu próprio SHA;
o teste real valida o fluxo de remoção, não PITR remoto ainda não configurado.

### 17/10, 00:11 UTC — continuidade após atualização de estado e plano final preservado

O estado do ambiente foi republicado como starting/ready, mas o heartbeat
continuou recente. A verificação somente leitura em 00:09:28 confirmou os
mesmos IDs, PIDs e StartedAt do app e PostgreSQL registrados antes do soak;
não houve restart provocado pelo agente. A validação final de toda a janela
ainda depende do encerramento, sem atribuir passe apenas a esse checkpoint.
O skill de onboarding foi consultado para revalidar o ambiente disponível;
não foi necessário reconfigurar ferramentas nem salvar um novo draft.

O [plano pré-declarado](results/stress/final-campaign-plan.json) foi ampliado
antes das próximas medições: o disco local congelado permanece baseline;
os dois passes finais de quatro horas por backend continuam obrigatórios
no novo candidato. Um baseline S3 ordinário novo no binário retido usará
o mesmo perfil de steady/capacidade, com soak diagnóstico de 600 s, apenas
para comparar as três amostras steady no protocolo atual. Não substitui
nenhuma endurance final nem equipara a janela curta a quatro horas. A suite
nativa terá um ELF para os doze casos, separado do build local de performance.

### 17/10, 00:26 UTC — CI da quinta correção concluído

O [recibo de CI](results/stress/ci-85b3c5a.json) registra todos os 23
check-runs de 85b3c5a: **18 success, quatro skipped e um failure**. Os skips
são os outros targets opcionais do workflow manual; não foram contados como
testes executados. O único failure é admin-ui, especificamente o step
`versioned dist is up to date`; checks/testes/build anteriores passaram.
O job Rust completo, incluindo Clippy, cargo test --all, onboarding, ciclo
de vida/restore e S3 real, passou, assim como CodeQL e seus quatro analisadores.
Assim, as quatro regressões Rust de backup_config estão verificadas no SHA
correspondente. O source final após assets ainda terá seu próprio CI.

### 17/10, 01:19 UTC — disco completo no protocolo atual e intervalo de preparação

A campanha disk-final-pass-1-002 passou com limpeza: 14.400 s contínuos,
4.032.001 operações úteis, zero drops/inesperados, 3.937 rejeições esperadas
e 257.500 eventos sem perdas/duplicações/extras. Os 62 relatórios de fases
fecharam os counters; app/PG preservaram identidade, 16 renovações e dois
ciclos de poda. Foram 3.283 amostras, gap11,0 s, census100k/100k/10.129 e
seis censos de descanso iguais (24FD/4tarefas/18sockets), zero transações
presas. É baseline ELF75/fonte2ef, não o novo candidato. O checkpoint012
chegou a 118.734 s (32h58min54s), excluindo warmup/preparo/descanso/parciais.

O CSV allowlisted foi exportado antes de qualquer compressão de raw. PNG/SVG
foram renderizados e inspecionados: a primeira execução teve aviso não fatal
de cache Fontconfig; ela e seu produtor foram retidos privadamente. Cache
XDG/MPL próprio corrigiu o aviso; segunda renderização manteve o mesmo PNG
SHA5569c46e, com recibo novo e nenhum aviso. Nenhum dado foi alterado.
A projeção de owners passou a computar igualdade, sem presumir recuperação.

A suíte Python024 executou290 testes com dois erros de Oxlint: o TMPDIR
que defini em target/ fez o engine ignorar fixtures e emitir aviso antes
do JSON. Reproduzido explicitamente com number_of_files0; não era defeito
do binário. TMPDIR próprio fora do repositório, no filesystem workspace,
resolveu. A suíte025 passou290/290 em14,987 s, sem ignorar testes nem alterar
assertions/regras. A probe HEAD404 de100iterações com GCdesativado manteve
5->5FD; sem defeito reproduzido, não foi feito patch nesse helper.

No intervalo, archive003 foi revalidado (SHA98248025,1.068.449.450B), assim
como os121 outputs debug restaurados: hashes, modos, mtime e hardlinks.
A remoção somente desses caches rebuildáveis liberou3.740.049.408B em6,986s.
Release-cache, dados, baseline e Git foram preservados. Debug checks precisam
restauração/rebuild; o build planejado é release-only. Nenhum Rustbuild/test
local ocorreu desde a restauração anterior, apenas rustfmt.

O crescimento anônimo observado (26,668->29,242MiB nas medianas de15min)
não recebeu atribuição automática a cache. O plano inclui verificação no
candidato e diagnóstico separado de quatro horas com alocador, se persistir;
instrumentação continua excluída das comparações ordinárias.


### Final local artifact and native suite dispatched (2026-10-17 01:25 UTC)

The separate tracked panel rebuild passed (`7e68fd2`). A clean locked Rust
1.99.0 release build then passed in 93.831 seconds with four Cargo jobs, thin
LTO, one codegen unit and no compiler/profile overrides. The retained local
ELF is 30,469,304 bytes, SHA-256
`b047fb880feb10e41e7d0308e6037b8cd3bc770b0452a30cab59c3e2d20514a0`.
The [build receipt](results/stress/final-local-candidate.json) records source,
manifest/lock hashes and product tree identities; build success is not stress
approval. No ordinary Rust/UI suite or build overlaps local measured traffic.

The predeclared `s3-retained-baseline-001` now runs separately on the retained
old ELF, obtaining comparable current-protocol steady measurements. Its
600-second soak cannot replace either mandatory final four-hour S3 pass.
[Native suite 38990677734](https://github.com/Nelcota/Nelcota/actions/runs/38990677734)
was dispatched on the same source with twelve fresh, serialized outer/guest
VM cases and its own common Actions-built ELF. Neither its completion nor
bit identity with the separate local build is assumed. Both final local
backend passes and all other remaining matrix cells stay open.


### Final-source ordinary CI and twelve native cases passed (2026-10-17 01:56 UTC)

All [eighteen ordinary checks](results/stress/ci-final-artifact-source.json)
on the frozen product source `7e68fd2` passed, including the panel asset drift
guard, Rust/real services, SDKs, supply chain and CodeQL. The original failed
asset/identity/CodeQL receipts remain unchanged.

[Native suite 38990677734](results/stress/native-final-suite/index.json)
completed all twelve serial cases successfully on fresh disposable outer/guest
VMs, with confirmed cleanup and no critical guard failure. Every case used
the same 30,452,336-byte Actions-built ELF, SHA-256
`010d0fc52f5cc5961f56baf6902294abfcc2c9d855ccbf7605e4b91e8a89a45c`.
The bounded original notice reports are preserved with their transport hashes.
Ten application cases contain twelve actual deployed-inode/running/provided
byte comparisons in total, including the two post-power-cut processes.
No bit identity with the separate local performance build is claimed.

Both native runtimes reconciled all thirty external ACKs with zero missing,
duplicate or extra operations and matching private file bytes. Restore took
2.014/2.376 seconds; guest boot recovery took 17.782/17.260 seconds. Both final
idle apps had 14 descriptors/ten sockets/three tasks. These functional results
do not certify in-flight cut writes, physical host power loss, fresh/off-host
restore, public ACME, PITR or endurance capacity. Preparation and overlapping
native/local observations do not increase the conservative effective-load
checkpoint. The retained S3 baseline remains active, and the four predeclared
final disk/S3 endurance campaigns are serialized behind verified closure;
failed workload/evidence/cleanup/comparison gates stop that chain.


### Retained S3 baseline interrupted; original failure preserved (2026-10-17 02:13 UTC)

The environment access transition coincided with external SIGTERM reaching
the frozen S3 campaign handler in `calibrate-75`. The sender is unverified;
no application bug or resource-guard breach is inferred. Owned teardown
completed, and the [failed receipt](results/stress/s3-retained-baseline-001-interrupted.json)
retains 36 closed phases/387 resource observations. Only 2,130 completed
seconds excluding warmup enter [checkpoint013](results/stress/load-time-checkpoint-013.json),
now 120,864 seconds (33h34m24s). Partial calibration, unexecuted soak and absent
final invariants cannot certify the baseline/candidate. The tool operator
exit receipt was lost; the generation-verified waiting chain was terminated
with pidfd before any final campaign started. That exited operator awaits
the environment PID1 reaper; no live fixture or child work remains.

Fresh baseline002 is predeclared with unchanged profile/limits/SLOs. A bounded
detached owner will serialize it and the four final endurance campaigns,
retain PID generations, reap only its own adopted descendants and stop on
any failed workload/evidence/cleanup/comparison gate. Per-campaign independent
watchdogs remain authoritative; a future observation gap still rejects a run.

A reversible archive also verified two older debug-test executables by full
round-trip hashes/modes/mtime. Their unlink did not increase actual workspace
free bytes, unlike the previously restored upper-layer cache003. No logical
size is presented as reclaimed headroom. The private 163,161,294-byte archive
and restoration manifest remain retained; no source, release ELF, data or
original failure was discarded. Ten-project startup reserves are unchanged.


### Retained S3 retry closed; first final disk candidate started (2026-10-17 03:47 UTC)

[Retained S3 baseline002](results/stress/s3-current-protocol-baseline.json)
passed 62 closed current-protocol phases, all three steady measurements per
workload, the 400/s declared qualification ceiling and a 600-second soak
at 280/s. It recorded 168,001 useful operations/171,036 physical HTTP/128
expected negatives, zero drops/unexpected errors, 64,248 reconciled events,
843 observations with 11.5-second maximum gap, final read-only census and
confirmed cleanup. Short idle FD counts 26–28 returned to 24 after 180s;
OS tasks stayed four. These are OS owners, not an exact Tokio-future or
heap-object census. Session renewal, two pruning cohorts and four-hour
endurance remain explicitly not run in this short baseline.

The dedicated short evidence collector corrected two preparation errors
(invalid taskset option and assuming a long-session canary file existed).
The private preparation-failure receipt is retained; the mandatory strict
four-hour summarizer was not relaxed.
[Checkpoint014](results/stress/load-time-checkpoint-014.json) excludes the
active candidate entirely and records 125,754 completed seconds (34h55m54s).

The bounded detached controller started the first final disk candidate only
after verified S3 closure. Its product tree still matches build source 7e68fd2
and immutable local ELF b047fb88; four-hour and regression approval remain
pending. The provider cgroup peak 225 tasks/617,418,752 memory.current bytes
adds an ownership investigation, not a cache/live-allocation conclusion.
Any alternative-worker diagnostic will be separate and cannot replace or
silently reconfigure the ordinary default-provider comparisons.
