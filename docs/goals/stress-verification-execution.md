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
