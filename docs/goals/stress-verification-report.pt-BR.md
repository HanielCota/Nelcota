# G05 — relatório técnico de stress

**Estado: em execução, sem aprovação do G05.** Este relatório é atualizado
durante as campanhas. Data deste checkpoint: 17/10/2026, 01:56 UTC.
Não houve merge nem deploy de produção. O trabalho está na branch
`test/g05-stress-verification`, no [PR #90](https://github.com/Nelcota/Nelcota/pull/90),
mantido como rascunho para revisão.

## Tempo efetivo e artefatos

O [checkpoint 012](results/stress/load-time-checkpoint-012.json) registra
**118.734 segundos, ou 32h58min54s, de carga efetiva concluída**. O cálculo
conservador exclui preparação, aquecimento, descanso, fases interrompidas,
controles artificiais e dupla contagem de janelas simultâneas. Tempo mínimo
de oito horas atingido não significa aprovação dos demais critérios.
O tempo parcial das campanhas ainda abertas não está somado nesse checkpoint.

| Artefato | Identidade e alcance |
|---|---|
| Código do backend sob carga local | `2efede8c27ee12119142170e8bd9742ff7cb900e` |
| SHA-256 do binário local preservado | `75c44c85a85db26195166aa1d2c6caf481e3db476801ec8131525ef62adf1be0` |
| Candidato local final congelado | `7e68fd21430ac93125149f60927f8676b077811d`; SHA-256 `b047fb880feb10e41e7d0308e6037b8cd3bc770b0452a30cab59c3e2d20514a0`; [proveniência](results/stress/final-local-candidate.json), [18 checks ordinários aprovados](results/stress/ci-final-artifact-source.json), carga final pendente |
| SHA-256 do workload k6 atual | `0ac01578bf19155302fe465884b480d792569086f23bb4ba3615ca5f0ce2547d` |
| Protocolo da repetição local em andamento | Snapshot imutável de `1f391594dd8db6cccac54d6720d90c33d461d1f1`; `disk-final-pass-1-002` concluiu como baseline no binário retido, com protocolo atual |
| Primeira execução nativa separada | [Actions 38962480993](https://github.com/Nelcota/Nelcota/actions/runs/38962480993), código congelado `e80770b3f6aa1f64d011a3d62f8ebf0f1201bdac`; reprovada no oráculo de signup antes do backup |
| Primeiro fluxo nativo completo | [Evidência](results/stress/native-vm-runtime-pass-1.json), código `7e1bd25b20a706c66d0be549c6f85851b6db843f`; aprovado no escopo funcional declarado |
| Candidato nativo com correção PostgreSQL | [Evidência](results/stress/native-vm-runtime-candidate-pass-1.json), código `6f8a3e727b85b869318c2a2712982293c0724125`; binário `0cccc84840b5d09ee76b1ab2005b52e7d702bd44c9f9c7662df5c6bde82e110f`, verificado dentro da VM |
| Correção de cwd, ativação nativa | [Antes](results/stress/native-default-root-before.json)/[depois](results/stress/native-default-root-after.json), código posterior `b43dc5802d6d236a3ed5b8c0253122ee12b8ae5e`; valida init/up/HTTPS; a migração no cwd/`NELCOTA_ROOT` teve defeito próprio corrigido e repetição com SQL confirmada |
| Propriedade de um host nativo por máquina | [Antes](results/stress/native-host-root-before.json)/[depois](results/stress/native-host-root-after.json), código posterior `f560493ae6bcd8c4c94d51b6d7d049a608b30de0`; binário `27a7fb004db277ec59bd071f0cba8f009e31f416d710cda7880daa3bc5c844ae` |

A execução local em andamento preserva o binário anterior como baseline.
A correção real da CLI exige novo build e repetição das campanhas afetadas;
o hash anterior não será apresentado como o artefato corrigido.

Cada campanha preserva manifesto de propriedade, limites, configuração,
hashes das ferramentas/imagens, fases, timestamps monotônicos, métricas e
recibo de limpeza. Os arquivos privados ficam fora do Git: credenciais,
cookies, URLs assinadas, dumps, SQL privado e payloads não são publicados.
O backend de produção continua um único binário Rust, sem Node ou Python.

## Situação dos 12 critérios

| # | Critério do G05 | Situação neste checkpoint | Evidência ou impedimento |
|---|---|---|---|
| 1 | Inventário, baseline imutável e SLOs declarados | Atendido para o staging local | [Inventário](results/stress/coverage-inventory.json), [plano](results/stress/final-campaign-plan.json), manifestos por execução. A matriz registra os cenários ainda incompletos. |
| 2 | Calibração, unidades, amostragem e oráculos | Validado nos controles corrigidos | Controles nativos de bytes, redirects, contadores esparsos, cgroups e compactação; campanhas finais ainda precisam terminar. |
| 3 | Cancelamento, watchdogs, redação e limpeza restrita | Demonstrado nos escopos local e nativo executados | [Controle nativo de guardas/pidfd](results/stress/native-guards-pidfd.json), interrupções reais e recibos de limpeza. A VM própria também confirmou limpeza e limites reais de kernel, sem guard crítico disparado nas execuções aprovadas. |
| 4 | Duas passagens de steady/staircase/picos/descanso | Pendente no protocolo final | Fases anteriores e falhas estão preservadas. A primeira repetição foi interrompida; sua substituta passou no protocolo atual como baseline. As duas passagens do novo artefato final ainda precisam executar. |
| 5 | Toda a matriz funcional, de segurança e concorrência | Parcial | CI funcional, primeiros browser/fault e smokes reais disponíveis; cobertura funcional não substitui stress repetido de cada célula. |
| 6 | Isolamento/fairness em 1/5/10 projetos e controle responsivo | Reprovado/incompleto | Primeira topologia de um projeto passou no protocolo anterior; cinco/dez projetos tiveram violações reais de fairness. Dez projetos também falharam no drain de 30 s. |
| 7 | Quatro horas contínuas em disco e S3 no artefato final | Pendente | Três execuções históricas completaram fisicamente quatro horas, com aprovação de contadores retirada. Uma nova campanha em disco passou no protocolo atual, como baseline; as campanhas de quatro horas do novo candidato ainda não executaram. |
| 8 | Falhas, drain, restart, restore e deployment aplicável | Parcial | Primeiro fault passou com reconciliação de ACKs e restore local novo. Dois fluxos nativos passaram no escopo de restore no mesmo alvo e perda da RAM da convidada. Repetição final de faults/carga, atualização operacional, DNS/ACME públicos e recuperação fora do host continuam incompletos. |
| 9 | Recursos, invariantes de dados/arquivos/eventos e regressão | Parcial | Invariantes locais e plateaus observados; precisam das campanhas finais comparáveis. Retenção anônima do provedor ainda exige atribuição. |
| 10 | Correções focadas e regressões dos defeitos confirmados | Verificado nos defeitos já reproduzidos; revisão continua | Antes/depois em cinco defeitos reais: cluster incorreto, unit relativa, outro host nativo, lease aninhado na migração e sucesso indevido de remoção com limpeza incompleta. Os cenários específicos passaram em VMs novas. A falha da fixture Rust teve local original recuperado e passou 1.000 repetições após I/O de prontidão; o erro original continua visível. Fairness não recebeu causa de produção inventada. |
| 11 | Rust/UI/SDK/lint/CI correspondentes ao SHA testado | Aprovado na fonte congelada `7e68fd2` | [18 checks ordinários](results/stress/ci-final-artifact-source.json), incluindo Rust/serviços reais, painel e assets, SDKs, supply chain e CodeQL. As 12 execuções nativas também passaram no mesmo source, com ELF próprio comum. Falhas anteriores de Rust, fixtures CodeQL e assets continuam preservadas; alterações futuras exigem a revalidação afetada. |
| 12 | Relatório, limites e todos os testes falhos/bloqueados/não executados | Em consolidação | Este relatório e o [registro detalhado](stress-verification-execution.md) preservam os resultados; ainda há campanhas abertas. |

## Staging e limites

O ambiente local oferece cinco slots de afinidade e quota efetiva de quatro
CPUs para o controlador. Containers filhos têm limites próprios; não se
atribui a eles um limite agregado do cgroup do controlador que não os contém.
App, PostgreSQL e gerador usam CPUs distintas; a telemetria usa outro slot,
e o S3 real usa o slot adicional. As campanhas são serializadas localmente.

O perfil ordinário usa app de 1 CPU/128 MiB, PostgreSQL de 1 CPU/384 MiB,
gerador de 1 CPU/512 MiB com limite Go de 448 MiB e Prometheus de
0,25 CPU/512 MiB. O S3 ordinário acrescenta 0,5 CPU/1.024 MiB.
Os manifestos de cada topologia detalham quotas, afinidades e oversubscription
quando declarada. Reserva mínima: 10% de RAM disponível e 15% de bytes/inodes
livres. Banco, storage e telemetria têm guardas físicos próprios; nenhuma
injeção enche o disco compartilhado. O PostgreSQL de testes alheio às
campanhas foi preservado.

O perfil de endurance usa disco de banco de 1.024 MiB, WAL de 256 MiB,
storage ordinário de 1.024 MiB e duração máxima de seis horas, incluindo
preparação/fases anteriores. O soak planejado é de 14.400 segundos contínuos,
a 70% da maior taxa qualificada no teto previamente declarado. O teto
de 400 iterações/s não é anunciado como capacidade máxima física.

A fixture maior usa um milhão de linhas em **cada** tabela products/private
e objetos de 1 MiB. Seu perfil próprio usa PostgreSQL de 2.048 MiB de disco,
storage em disco de 1.024 MiB e storage S3 de 4.096 MiB. A mudança do orçamento
S3 foi declarada antes da nova medição, após a falha física original, e está
condicionada às reservas. Seu soak de dez minutos não substitui endurance.

## Defeitos confirmados e resultados antes/depois

As correções desta campanha atingem a confiabilidade do executor e
falhas reais de isolamento e inicialização operacional na CLI nativa. Não foi demonstrada melhoria
de desempenho do backend de produção por essa correção de segurança.

| Problema confirmado | Antes | Correção e verificação |
|---|---|---|
| Respostas semanticamente inválidas podiam contar como trabalho útil | Controles com leitura vazia/filtro incorreto/JSON inválido e outras respostas indevidas eram aceitos pelo workload anterior | Oráculos de conteúdo, identidade, ordem, expiração e bytes; [controle antes/depois](results/stress/semantic-oracle-before-after.json) preserva cada resultado esperado, inclusive as rejeições. |
| Comparação podia aprovar amostras iniciais de uma campanha posteriormente falha | Autocomparação real retornava sucesso | Comparador exige conclusão e limpeza válidas; [controle](results/stress/comparator-failed-run-before-after.json) passa de exit 0/aprovado para exit 1/rejeitado. Isso não é ganho de performance do produto. |
| Leitores SQLite permaneciam abertos até GC | 35 aberturas elevaram descritores de 4 para 39 | Fechamento explícito e regressões; mesmas 35 leituras mantiveram 4 descritores com GC desativado e bytes do banco intactos. [Evidência](results/stress/sqlite-archive-owner-before-after.json). |
| Consulta final de drops esparsos podia inventar zero | Segundo disco registrou zero; observador suplementar reteve 1.127 drops | Observação contínua por cenário, limites de cardinalidade, rejeição de resets/lacunas e prova fechada exigida nos comparadores. Controle nativo reteve 538 após expiração das consultas instantânea/range. [Errata](results/stress/sparse-drop-staleness-finding.json), [controle](results/stress/native-arrival-control-before-after.json). |
| HTTP físico e rejeições negativas também expiravam | Cada endurance histórico reportou zero negativos; auditoria de assertions recuperou 3.937 | Contadores por série/status com cobertura fechada e reconciliação independente; controle reteve 273 requisições, incluindo três 401, após expiração das séries. [Auditoria](results/stress/historical-http-counter-audit.json), [controle](results/stress/native-http-control-before-after.json). |
| Timeout de `docker stats` interrompia uma observação recuperável | Fleet de cinco projetos parou após timeout de 10 s | Fallback somente para timeout lê cgroup real do container, com CID, PID e geração/propriedade verificados; controle nativo confirma memória/tarefas/CPU/OOM. Não amplia o limite de lacuna de 30 s. Repetição da topologia permanece obrigatória. |
| Latência incluía apenas o último hop | Download com atraso inicial de 250 ms reportava p95 de 6,165 ms | Relógio monotônico mede a requisição lógica inteira; mesmo controle passa a 256,798 ms, com oito downloads verificados por lado. [Evidência](results/stress/native-logical-latency-before-after.json). Os valores maiores corrigem submedição, não representam regressão de servidor. |
| Segmento de bloco TSDB fazia reserva física não limitada por WAL/retenção | Primeira repetição final parou em 522 MiB contra guarda de 512 MiB | Segmentos explícitos de 16 MiB e caminho próprio comum aos executores; guarda continua em 512 MiB. Controle 004 confirmou flags, 2.000 incrementos e quatro snapshots reais de 1.004 séries cada, com limpeza. [Antes/depois](results/stress/native-metrics-compaction-before-after.json). |
| Novo oráculo nativo esperava o status incorreto no signup | VM real retornou o `201` documentado, mas o executor exigia o `200` genérico e parou | Exigência exata de `201`, token presente e identidade correta, com regressão de respostas HTTP. Repetição em VM nova aprovada; não se alterou a API. [Antes](results/stress/native-vm-runtime-first-failure.json), [depois](results/stress/native-vm-runtime-pass-1.json). |
| Resultado nativo foi truncado na anotação do Actions | Mensagem cortada em 4.096 caracteres; uma primeira fragmentação também excedeu dez anotações por step | Transporte gzip/base64, no máximo nove fragmentos mais resumo, SHA-256 e regressões de reconstrução. Os relatórios originais foram recuperados integralmente pelo workflow, preservando os veredictos. Download local de artifacts continua indisponível. |
| Administração nativa podia alterar outro cluster PostgreSQL | Com `17/main:5433` e `17/sentinel:5432`, `init` retornou sucesso e mudou a senha do cluster sentinela | Preflight rejeita layout incompatível antes de pacotes/host/config; clientes administrativos são vinculados a `17/main` e ao socket/porta esperados; senha segue por stdin. Seis regressões Rust e a [VM antes](results/stress/native-postgres-collision-before.json)/[depois](results/stress/native-postgres-collision-after.json) comprovam a fronteira. Fluxo nativo normal corrigido também passou. |
| Instalação no diretório atual gerava unit inválida em runtime | `init` exit 0, `EnvironmentFile` relativo, `up` timeout e 14 reinícios, apesar de `systemd-analyze verify` exit 0 | Diretório resolvido antes de gerar configuração. VM nova confirmou caminho absoluto, init/up/verify exit 0 e HTTPS válido; 98,166 s com preparo, 19 amostras e limpeza. A extensão de migrações revelou um quarto defeito, descrito abaixo; o caminho de ativação permanece validado no escopo original. |
| Outro diretório podia tomar a instalação nativa | Segundo init com `--skip-checks` exit 0 mudou senha/Caddy e criou outro manifesto/unit | Lease da máquina e owner persistente antes de provisionar; fronteira de diretório e liberação somente após remoção completa. Na VM nova: rejeição específica exit 1, senha/Caddy/linha inalterados, nenhum segundo manifesto/unit; 72,177 s com preparo, 14 amostras e limpeza. Rust de `f560493`/`3b56b25` passou; remoção real confirmou owner liberado e recusa de `up` no host vazio. |


| Remoção nativa declarava sucesso apesar da configuração de backup não removida | `remove --no-backup` exit 0, unit/cluster apagados e owner liberado apesar do erro de unlink | Validação do caminho antes de destruir o runtime, propagação de erros e mesma regra na desativação do PITR. [Antes](results/stress/native-remove-boundary-before.json)/[depois](results/stress/native-remove-boundary-after.json): exit 1 específico, owner/manifest/cluster/linha preservados, seguido de remoção normal com backup bem-sucedida. As exclusões do sentinel eram solicitadas pelo probe; não foram classificadas como perda não intencional de ACKs. PITR real remoto continua não certificado. |
| Migração nativa no cwd/`NELCOTA_ROOT` tentava adquirir o próprio lease | Dois comandos reais retornavam exit 1/`deployment_busy`, com o lease externo corretamente mantido | Filho nativo recebe `--dir` explícito do projeto, preservando o lease do host no pai. [Antes](results/stress/native-cwd-migration-before.json)/[depois](results/stress/native-cwd-migration-after.json): ambos exit 0, tabela e journal presentes, uma linha em SQL. [Errata](results/stress/native-cwd-migration-census-erratum.json) retira a alegação antiga de ausência de tabela, pois o primeiro census comparava bytes/string; não retira os exits 1/busy observados. |
| Census nativo podia negar presença SQL válida | Repetição já tinha migrate exit 0, mas o oráculo interpretava `b"t"` como falso e não contava linhas | Parser de bytes estrito, presença de tabela/journal separada e COUNT decimal limitado. Três regressões; VM nova passou com COUNT 1. A [execução intermediária](results/stress/native-cwd-migration-census-failure.json) continua reprovada. |
| Descoberta de alocador ignorava o prefixo real e não publicava arquivo legível | 17 exports reais continham `rustfs_memory_allocator_`, mas o controle contou zero; uploader falhou no JSON 0600 | Prefixo corrigido, disponibilidade explícita separada de valor e publicação validada 0644. [Primeira falha](results/stress/provider-memory-discovery-first-failure.json) preservada; [repetição corrigida](results/stress/provider-memory-discovery-after.json) passou com 17 exports e três gauges de memória, inclusive publicação. Bytes solicitados vivos continuam indisponíveis e owners não verificados. |

Outras mudanças de propriedade, filesystem, fault injection, inicialização
de clientes, browser, ingress e teardown estão descritas por commit no
[registro de execução](stress-verification-execution.md). Falhas de embalagem
ou de oráculo dos controles continuam registradas como falhas originais;
uma análise posterior não reescreve seus veredictos.

## Resultados de carga, integridade e recursos

| Campanha | Resultado observado | Limite da conclusão |
|---|---|---|
| Disco histórico 1 | 14.400 s contínuos, 4.031.806 operações úteis e 248.381 eventos reconciliados | Aprovação final retirada por contadores esparsos; não certifica protocolo atual. |
| S3 histórico 1 | 14.400 s contínuos, 4.031.965 operações úteis, 249.824 eventos reconciliados, 16 renovações de sessão e dois ciclos de poda | RustFS real local; aprovação dos contadores retirada. Não representa AWS, rede remota ou durabilidade fora do host. |
| Disco histórico 2 | 14.400 s contínuos, 4.030.874 operações úteis e 249.795 eventos reconciliados | 1.127 drops retidos por observador suplementar; protocolo original insuficiente, mantido como diagnóstico. |
| Disco final 001 | 4.290 s efetivos concluídos, 833 amostras, calibração de 400/s repetida | Interrompido automaticamente pela reserva TSDB de 522 MiB. Soak parcial excluído do tempo efetivo; invariantes finais não alcançadas; limpeza confirmada. |
| Disco no protocolo atual, baseline | 14.400 s contínuos, 4.032.001 operações úteis, 4.035.938 requisições do tráfego principal, 3.937 negativas esperadas, zero drops/erros inesperados; 257.500 eventos reconciliados | Binário retido `75c44c85`, fonte `2efede8`; 3.283 amostras, gap máximo observado de 11,0 s, 16 renovações, dois ciclos de poda e limpeza. Não certifica o novo artefato. |
| Um projeto/Caddy | 2.490 s de carga, 506 amostras, 32/s qualificados e nominal de 25/s, 2.928 eventos reconciliados | Primeira execução no protocolo anterior de latência/telemetria; requer duas passagens finais comparáveis. |
| Cinco projetos compartilhados, smoke | Todos os cinco cenários concentrados falharam em fairness; RLS saudável p95 chegou a 217,638 ms contra meta de 200 ms | 1.129 eventos reconciliados e drain/limpeza aprovados não promovem o resultado de desempenho. |
| Cinco projetos compartilhados, completo | 2.790 s concluídos, 604 amostras, 24 falhas prévias de latência em noisy-mixed | Interrompido em noisy-login por timeout do observador; invariantes finais não alcançadas, limpeza confirmada. |
| Dez projetos, duas fixtures independentes | 3.990 s efetivos por execução; três medidas balanceadas e reload Caddy passaram a 4/s por projeto | Fairness reprovada em todos os cinco cenários concentrados; drain de 30 s terminou com 5.834/7.010 pendências no projeto quente. Invariantes finais não alcançadas. |
| Browser/SDK 1 | Três abas, 511 navegações reais; p95 visual de 399,4 ms; 300 clientes descartados coletados; upload/cancelamento/renovação reais | Primeira passagem Chromium local, sem sandbox interno do Chromium; segunda passagem pendente. Isolamento externo do container permanece ativo. |
| Fault/recovery 1 | 53.049 ACKs reconciliados, 2.938 respostas reais pós-COMMIT descartadas; zero ACK perdido/corrupção observada | Primeira passagem; respostas ambíguas foram consultadas, sem retry automático de mutações. |
| Runtime nativo corrigido | Inicialização, migrations repetíveis, HTTPS validado, RLS, backup/restore e boot após interrupção da VM; 30 ACKs externos reconciliados sem perdas/duplicações/extras | 168,203 s incluindo preparo, 32 amostras, limpeza confirmada. Restore no mesmo alvo de 2,108 s e recuperação da convidada de 16,367 s; sem escrita em andamento no corte e sem alegação de durabilidade fora do host. |
| Relógio em VM descartável | Passo observado de 1.018,326 s, JWT expirado 401, serviço preservado/anônimo negado; relógio/NTP restaurados, offset final 0,471 s e acesso ordinário recuperado | [Evidência](results/stress/native-clock-drift-pass-1.json), sem alterar o relógio do host/controlador e sem crédito de endurance. |
| Contratos de instalação nativa | Caminho de ativação no cwd e recusa de outro diretório passaram em VMs próprias | Não substituem a repetição completa de runtime após todas as correções; cwd/`NELCOTA_ROOT` migrate agora passou com tabela, journal e contagem SQL confirmados. |

A [campanha atual em disco](results/stress/disk-current-protocol-baseline.json)
fechou **62 fases** com contadores contínuos HTTP/arrivals, 400 iterações/s
qualificadas em passos de cinco minutos no teto pré-declarado e soak a
280/s (70%). O teto de 400/s continua sendo limite da investigação, sem
alegação de capacidade máxima física. App e PostgreSQL preservaram seus
processos durante os 14.400 s. O census read-only confirmou 100.000 linhas
products, 100.000 private, 10.129 objetos e zero transações ociosas abertas.
Invariantes finais: zero pendências, assinaturas inválidas, eventos perdidos,
duplicados ou extras; 129 objetos verificados e 10.129 objetos físicos com
2.195 bytes. SQLite fechado passou integrity_check sem sidecars.

Os cinco descansos após picos e o descanso final de 180 s tiveram os mesmos
**24 descritores, quatro tarefas e 18 sockets**, sem transações presas/locks.
RSS máximo geral: 49,1 MiB; no soak: 47,4 MiB. Disco PostgreSQL: máximo
452 MiB. Não houve OOM. A mediana dos primeiros/últimos 15 minutos do soak
subiu de **44,824 para 47,395 MiB de RSS** e de **26,668 para 29,242 MiB
anônimos**. Essa elevação ainda exige atribuição; censos estáveis de recursos
não provam ausência absoluta de vazamento de heap. A repetição no candidato
e, se persistir, um diagnóstico de alocação de quatro horas foram mantidos
no plano, separado das comparações ordinárias.

![Recursos medidos no baseline atual](results/stress/disk-current-protocol-baseline-resources.png)

[CSV numérico](results/stress/disk-current-protocol-baseline-resources.csv.gz),
[índice/checksums](results/stress/disk-current-protocol-baseline-resources.csv.index.json),
[SVG](results/stress/disk-current-protocol-baseline-resources.svg) e
[recibo da figura](results/stress/disk-current-protocol-baseline-resources.json)
preservam o encadeamento entre observações, harness e binário. A figura
mostra tempo de observação, incluindo aquecimento/descanso, que não substitui
o tempo efetivo de carga do checkpoint.

No fault pass, o restore em alvo local novo levou **2,636 s**, com RPO
superior medido de **17,997 s**. O snapshot tinha 49.692 operações contra
50.338 na origem: as 646 posteriores representam rewind intencional do
restore, e não perda não intencional de ACKs na origem. O backup foi realizado
com writers SQL controlados e arquivos imutáveis; não certifica todo cenário
de escrita concorrente de objetos, PITR ou restauração física fora do host.

Os plateaus observados do app histórico chegaram a 24 descritores, quatro
tarefas e 18 sockets após o descanso. O diagnóstico separado de uma hora
com glibc distinguiu cerca de **1,97 MiB em uso** de **24,00 MiB livres**
retidos nas arenas. O controle nativo incluiu alocação/liberação por worker;
não houve `malloc_trim` forçado para disfarçar RSS. Instrumentação de
alocação fica fora das comparações ordinárias de performance.

O provedor real apresentou memória anônima de cerca de 415 MiB no trecho
final histórico, sem atribuição completa entre MiMalloc, caches e objetos
vivos. Não se declara ausência absoluta de vazamento. O diagnóstico físico
de trash mostrou cerca de 17 MiB após DELETE, zero objetos lógicos, e
reclamação automática até 1 MiB com namespace de trash vazio após mais de
330 s. As falhas originais dos oráculos 002/003 foram preservadas; o
[verificador posterior](results/stress/native-s3-trash-reclamation.json)
validou as observações congeladas sem forçar GC. Essa conclusão física não
explica toda a retenção de memória anônima. A descoberta remota com o digest
exato recebeu 17 exports OTLP autenticados e observou gauges MiMalloc de
reserved/committed/page-committed. A flag
`malloc_requested_bytes_available = 0` impede interpretar requested-total como
bytes vivos. O próprio controle teve duas falhas; a repetição corrigida passou no escopo
de descoberta/publicação. O relatório original permanece bloqueado e o job original vermelho.

## Commits de referência, validação e pendências

| Commit | Mudança com evidência correspondente |
|---|---|
| `948f519` | Comparações dependem de trabalho útil e conclusão válida. |
| `5829717` | Oráculos semânticos antes da certificação de carga. |
| `c1e40bf` | Fechamento dos leitores SQLite. |
| `f899aca` / `d4ea5bc` | Retenção contínua de drops e controle real de expiração. |
| `c276cc3` | HTTP/rejeições esparsas e fallback de cgroup com propriedade. |
| `b82699f` | Latência da requisição lógica completa. |
| `c769934` | Preservação/análise da reclamação física S3. |
| `1f39159` | Reserva TSDB limitada, perfil estrito e exportação numérica. |
| `22acf21` / `e80770b` / `7e1bd25` | Inventário/execução nativa KVM, correção do oráculo de criação e publicação íntegra de evidências. |
| `0028307` / `d0927b5` | Isolamento dos clientes administrativos PostgreSQL, preflight de clusters e seis regressões; ordenação do módulo de testes e tradução do guia. |
| `6767e15` / `dec797d` / `6f8a3e7` / `77d2335` | Reprodução real da colisão, evidências recuperáveis e verificadas, retenção do binário e probes dos contratos de instalação. |
| `b43dc58` | Correção do caminho absoluto do host, com regressão real de ativação aprovada. |
| `f560493` / `76af3a4` | Propriedade nativa persistente e leases de criação/mutação; revisão da liberação por hosts vazios. |
| `14725d6` / `96c875d` | Migração nativa por diretório do projeto e census SQL tipado; antes/depois e falha intermediária preservados. |
| `3b56b25` | Controle real de clock drift com restauração em finally e watchdog monotônico. |
| `9e1f236` / `4ffad18` | Descoberta limitada do alocador real; prefixo, disponibilidade e permissão do artefato corrigidos. |
| `85b3c5a` | Preflight e propagação de erros na limpeza nativa de pgBackRest/PITR; falha e repetição real de remoção preservadas. |
| `c98aaaf` / `5fab62a` | I/O de prontidão da fixture de identidade; 1.000/1.000 repetições reais aprovadas. |
| `ffae0e5` | Suíte manual de seis cenários nativos em duas passagens com o mesmo ELF; as 12 execuções finais passaram na fonte `7e68fd2`, com verificação do processo ativo. |
| `a9bb8c2` | Diagnóstico CI HTTPS limitado, sem encaminhamento de token nem publicação de assertions. |
| `e4e89d3` / `5abdf2a` | Fixtures aleatórias e distinção entre candidato de validação e credencial efetiva; segunda falha preservada e CodeQL posterior aprovado. |

O [CI completo registrado de `85b3c5a`](results/stress/ci-85b3c5a.json)
mostra 18 checks aprovados, quatro jobs opcionais ignorados e uma falha, no
guard de assets do painel. As etapas de check/test/build do painel e o job
Rust completo (com as quatro regressões novas de limpeza) passaram.
CodeQL também passou. Depois do rebuild separado dos assets, os [18 checks ordinários de `7e68fd2`](results/stress/ci-final-artifact-source.json) passaram; o registro reprovado anterior continua preservado.

Os 290 testes Python atuais passaram em 14,987 s. A execução anterior
teve dois erros porque TMPDIR estava em target/, ignorado pelo Oxlint:
o engine informou zero arquivos e seu aviso precedeu o JSON. O temporário
foi movido para uma pasta própria fora do repositório; não se alteraram
assertions, regras ou binário do lint. O log reprovado foi preservado.
Os 246 testes Python anteriores ao executor nativo passaram em 13,399 s.
Os doze testes focados do executor nativo corrigido passaram em
0,012 s, sem serviços locais. A falha inicial da fixture do mock também foi
registrada no diário. O CI de `1f39159` concluiu seus 18 checks
com sucesso. No CI de `6f8a3e7`, Rust/Clippy, upgrade, SDKs, lint e supply chain
passaram; o job do painel falhou apenas no guard de assets versionados após seus
checks, testes e build. O rebuild local de `dist` foi reservado para o
intervalo entre campanhas, conforme as regras de recursos do repositório.
O check agregado do CodeQL também reprovou: anotou uma fixture estática
inválida de senha na nova regressão Rust como valor criptográfico fixo.
O teste passa a gerar a base aleatória e acrescentar os caracteres inválidos,
preservando a rejeição e exigindo seu erro específico. A primeira mudança
ainda reprovou; o ajuste posterior passou no check agregado e na análise Rust
de `f560493`, com a regra habilitada. O endpoint completo de alertas continua
HTTP 403, portanto não se afirmou uma contagem de todos os alertas da branch.
Uma falha anterior de Rust no CI e a repetição bem-sucedida no
mesmo SHA foram preservadas; downloads de logs retornaram HTTP 403, por isso
não se atribuiu uma causa nem se inventou contagem de testes daquele job.
O teste `running_native_identity_rejects_replacement_even_with_the_same_hash`
reprovou em `14725d6`; Rust em `3b56b25`, com o mesmo código relevante, passou,
sem comprovar a causa da falha original. O novo downloader seguro recuperou
[local/categoria do panic](results/stress/ci-native-executable-panic-location.json):
primeira validação na linha 863, identidade inicial diferente. A fixture agora
exige I/O real do filho antes de observar /proc; [mil repetições reais passaram](results/stress/ci-native-executable-identity-after.json)
em 9,064 s, sem alterar a checagem de produção ou somar crédito de endurance. Dois diagnósticos remotos limitados anteriores também falharam; o último publicou exit 1
sem código HTTP identificável. Esses recibos não fornecem o diagnóstico
original de compilação e não foram apresentados como testes aprovados.

Permanecem em execução/pendentes as duas campanhas finais por backend,
fixtures maiores completas por backend, topologias finais 1/5/10 com
repetições, segundo browser e segundo fault, comparações finais e toda
célula restante da matriz. Ainda não há autorização para declarar esses
testes aprovados a partir de smokes, CI funcional ou resultados históricos.

AWS/S3 remoto, restauração fora do host, PITR configurado, ARM64, gerador em
outra máquina e DNS/ACME públicos seguem sem alvo configurado. A VM KVM
tem uma execução inicial reprovada por oráculo, dois fluxos completos
aprovados em seus próprios hashes, cinco defeitos reais reproduzidos com regressões
aprovadas em seus escopos e controle real de relógio aprovado. A [suíte final](results/stress/native-final-suite/index.json) concluiu **12/12 casos**,
os seis cenários em duas passagens próprias, na fonte `7e68fd2`.
Seu ELF comum `010d0fc52f5cc5961f56baf6902294abfcc2c9d855ccbf7605e4b91e8a89a45c`
foi conferido em todos os casos. Houve 12 verificações de inode/digest
do processo ativo, incluindo depois dos dois cortes de energia da VM.
Todos confirmaram limpeza e ausência de guard crítico disparado.
Os dois runtimes reconciliaram 30 ACKs cada, com zero perdas/duplicados/extras.
RTO de restore: **2,014 e 2,376 s**; recuperação da convidada: **17,782 e
17,260 s**. App em repouso, nas duas VMs: **14 FDs, dez sockets, três tarefas**.
Esses tempos incluem um teste funcional pequeno; preparação nativa e
janelas simultâneas não aumentaram o checkpoint de carga efetiva local.
As duas passagens de endurance do candidato local continuam pendentes.
A perda de RAM da convidada não certifica perda de energia do host físico. Nenhum desses
impedimentos foi substituído silenciosamente por mock ou critério menor.

As campanhas continuam enquanto houver trabalho relevante e condições
seguras. O G05 permanece aberto enquanto houver requisito obrigatório
reprovado, bloqueado ou não executado.
