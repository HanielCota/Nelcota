# G05 — relatório técnico de stress

**Estado: em execução, sem aprovação do G05.** Este relatório é atualizado
durante as campanhas. Data deste checkpoint: 16/10/2026, 20:41 UTC.
Não houve merge nem deploy de produção. O trabalho está na branch
`test/g05-stress-verification`, no [PR #90](https://github.com/Nelcota/Nelcota/pull/90),
mantido como rascunho para revisão.

## Tempo efetivo e artefatos

O [checkpoint 011](results/stress/load-time-checkpoint-011.json) registra
**100.044 segundos, ou 27h47min24s, de carga efetiva concluída**. O cálculo
conservador exclui preparação, aquecimento, descanso, fases interrompidas,
controles artificiais e dupla contagem de janelas simultâneas. Tempo mínimo
de oito horas atingido não significa aprovação dos demais critérios.
O tempo parcial das campanhas ainda abertas não está somado nesse checkpoint.

| Artefato | Identidade e alcance |
|---|---|
| Código do backend sob carga local | `2efede8c27ee12119142170e8bd9742ff7cb900e` |
| SHA-256 do binário local preservado | `75c44c85a85db26195166aa1d2c6caf481e3db476801ec8131525ef62adf1be0` |
| SHA-256 do workload k6 atual | `0ac01578bf19155302fe465884b480d792569086f23bb4ba3615ca5f0ce2547d` |
| Protocolo da repetição local em andamento | Snapshot imutável de `1f391594dd8db6cccac54d6720d90c33d461d1f1`, fixture nova `disk-final-pass-1-002` |
| Primeira execução nativa separada | [Actions 38962480993](https://github.com/Nelcota/Nelcota/actions/runs/38962480993), código congelado `e80770b3f6aa1f64d011a3d62f8ebf0f1201bdac`; reprovada no oráculo de signup antes do backup |
| Repetição nativa em andamento | [Actions 38963816198](https://github.com/Nelcota/Nelcota/actions/runs/38963816198), código congelado `7e1bd25b20a706c66d0be549c6f85851b6db843f`; VM nova e oráculo corrigido |

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
| 3 | Cancelamento, watchdogs, redação e limpeza restrita | Demonstrado no staging local | [Controle nativo de guardas/pidfd](results/stress/native-guards-pidfd.json), interrupções reais e recibos de limpeza. Nova VM tem validação própria pendente. |
| 4 | Duas passagens de steady/staircase/picos/descanso | Pendente no protocolo final | Fases anteriores e falhas estão preservadas. A primeira repetição final foi interrompida; sua substituta está em execução. |
| 5 | Toda a matriz funcional, de segurança e concorrência | Parcial | CI funcional, primeiros browser/fault e smokes reais disponíveis; cobertura funcional não substitui stress repetido de cada célula. |
| 6 | Isolamento/fairness em 1/5/10 projetos e controle responsivo | Reprovado/incompleto | Primeira topologia de um projeto passou no protocolo anterior; cinco/dez projetos tiveram violações reais de fairness. Dez projetos também falharam no drain de 30 s. |
| 7 | Quatro horas contínuas em disco e S3 no artefato final | Pendente | Três execuções completaram fisicamente quatro horas, mas a aprovação dos contadores foi retirada após auditoria. Nenhuma substituta final foi aprovada neste checkpoint. |
| 8 | Falhas, drain, restart, restore e deployment aplicável | Parcial | Primeiro fault passou com reconciliação de ACKs e restore local novo; repetição e aceitação operacional nativa pendentes. |
| 9 | Recursos, invariantes de dados/arquivos/eventos e regressão | Parcial | Invariantes locais e plateaus observados; precisam das campanhas finais comparáveis. Retenção anônima do provedor ainda exige atribuição. |
| 10 | Correções focadas e regressões dos defeitos confirmados | Atendido para os defeitos de medição já corrigidos | Tabela abaixo com antes/depois. Falhas de fairness não receberam causa de produção inventada. Novos defeitos poderão exigir outras correções/repetições. |
| 11 | Rust/UI/SDK/lint/CI correspondentes ao SHA testado | Parcial | [18 checks de `1f39159`](results/stress/ci-1f39159.json) passaram. Alterações nativas posteriores têm CI próprio em acompanhamento. |
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

As correções desta campanha atingem principalmente a confiabilidade do
executor e de suas evidências. Não há, neste checkpoint, correção de produção
do backend cuja melhoria de desempenho tenha sido demonstrada.

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
| Novo oráculo nativo esperava o status incorreto no signup | VM real retornou o `201` documentado, mas o executor exigia o `200` genérico e parou | Exigência exata de `201`, token presente e identidade correta, com regressão de respostas HTTP. Repetição em VM nova em andamento; não se alterou a API. |
| Resultado nativo foi truncado na anotação do Actions | Mensagem cortada em 4.096 caracteres, com recibo final inacessível; download retornou HTTP 403 | Fragmentos limitados, numerados e com SHA-256; teste reconstrói relatório longo sem perda. O workflow relê o artifact público original dentro do GitHub para recuperar a evidência sem reescrever o veredito. |

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
| Um projeto/Caddy | 2.490 s de carga, 506 amostras, 32/s qualificados e nominal de 25/s, 2.928 eventos reconciliados | Primeira execução no protocolo anterior de latência/telemetria; requer duas passagens finais comparáveis. |
| Cinco projetos compartilhados, smoke | Todos os cinco cenários concentrados falharam em fairness; RLS saudável p95 chegou a 217,638 ms contra meta de 200 ms | 1.129 eventos reconciliados e drain/limpeza aprovados não promovem o resultado de desempenho. |
| Cinco projetos compartilhados, completo | 2.790 s concluídos, 604 amostras, 24 falhas prévias de latência em noisy-mixed | Interrompido em noisy-login por timeout do observador; invariantes finais não alcançadas, limpeza confirmada. |
| Dez projetos, duas fixtures independentes | 3.990 s efetivos por execução; três medidas balanceadas e reload Caddy passaram a 4/s por projeto | Fairness reprovada em todos os cinco cenários concentrados; drain de 30 s terminou com 5.834/7.010 pendências no projeto quente. Invariantes finais não alcançadas. |
| Browser/SDK 1 | Três abas, 511 navegações reais; p95 visual de 399,4 ms; 300 clientes descartados coletados; upload/cancelamento/renovação reais | Primeira passagem Chromium local, sem sandbox interno do Chromium; segunda passagem pendente. Isolamento externo do container permanece ativo. |
| Fault/recovery 1 | 53.049 ACKs reconciliados, 2.938 respostas reais pós-COMMIT descartadas; zero ACK perdido/corrupção observada | Primeira passagem; respostas ambíguas foram consultadas, sem retry automático de mutações. |

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
explica toda a retenção de memória anônima.

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

Os 246 testes Python anteriores ao executor nativo passaram em 13,399 s.
Os onze testes focados do executor nativo corrigido passaram em
0,006 s, sem serviços locais. A falha inicial da fixture do mock também foi
registrada no diário. O CI de `1f39159` concluiu seus 18 checks
com sucesso. Compilação, regressões e CI do código posterior continuam
separados. Uma falha anterior de Rust no CI e a repetição bem-sucedida no
mesmo SHA foram preservadas; downloads de logs retornaram HTTP 403, por isso
não se atribuiu uma causa nem se inventou contagem de testes daquele job.

Permanecem em execução/pendentes as duas campanhas finais por backend,
fixtures maiores completas por backend, topologias finais 1/5/10 com
repetições, segundo browser e segundo fault, comparações finais e toda
célula restante da matriz. Ainda não há autorização para declarar esses
testes aprovados a partir de smokes, CI funcional ou resultados históricos.

AWS/S3 remoto, restauração fora do host, PITR configurado, ARM64, gerador em
outra máquina e DNS/ACME públicos seguem sem alvo configurado. A VM KVM
tem uma primeira execução reprovada e uma repetição em andamento: a perda de RAM da
convidada não certifica perda de energia do host físico. Nenhum desses
impedimentos foi substituído silenciosamente por mock ou critério menor.

As campanhas continuam enquanto houver trabalho relevante e condições
seguras. O G05 permanece aberto enquanto houver requisito obrigatório
reprovado, bloqueado ou não executado.
