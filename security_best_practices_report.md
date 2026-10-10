# Revisão de performance, segurança e sanitização

Auditorias: 09/10 e 10/10/2026. Correções: 10/10/2026. Projeto: `D:/Nelcota`.

## Correções da quarta revisão — 10/10/2026

Os achados **18, 19, 20 e 21 receberam correções e testes de regressão**.
Os detalhes da revisão abaixo registram o comportamento anterior a essas
correções.

| ID | Correção | Validação específica |
|---|---|---|
| 18 | Grants de arquivo exigem `exp > now` antes e depois da consulta do objeto; o presign herda somente o tempo restante, sem elevar zero para um segundo | Fronteiras exatas de tempo e claims malformados; URLs vencidas há 0, 1, 10 e 29 segundos recusadas em disco e S3; URLs reais de duração curta deixam de servir/redirecionar após o prazo |
| 19 | O callback reivindica o estado atomicamente, apagando o verifier do banco antes de HTTP; aplica uma quota por IP separada de authorize | 32 callbacks concorrentes iniciam uma única troca; duplicatas não anulam o código válido; falha e cancelamento exigem novo authorize; quota retorna 429 com Retry-After; novo fluxo continua funcionando |
| 20 | A prévia recusa Content-Length acima do orçamento e lê o stream contando bytes; excesso e fechamento cancelam a leitura | Resposta de 8 MiB recusada sem `Response.blob`; streams sem tamanho ou com tamanho subestimado cancelados após exceder 1 MiB em um chunk; fronteira exata, bytes/tipo, erros de rede, fechamento e reabertura verificados |
| 21 | Capacidade de 50 mil chaves é efetiva; só a admissão de chave nova pode limpar, no máximo uma vez por segundo; chaves só são removidas após um minuto desde a última tentativa aceita | 50 mil chaves mais 300 tentativas não aumentam o conjunto; quotas ativas preservadas; recuperação libera espaço; 32 admissões concorrentes respeitam a capacidade de oito chaves usada no teste |

Os testes de regressão de expiração, concorrência OAuth, prévia e capacidade
falharam contra o comportamento anterior e passaram após as respectivas
correções. Foram adicionados **21 testes permanentes**: oito Rust, nove
unitários da UI e quatro de navegador. Os testes existentes de URLs
assinadas em disco/S3 e de duplicata OAuth também receberam novas asserções.

Implementação: [prazo do grant](D:/Nelcota/crates/storage/src/signing.rs:65),
[reivindicação OAuth](D:/Nelcota/crates/auth/src/oauth/flow.rs:61),
[quota do callback](D:/Nelcota/crates/auth/src/handlers.rs:233),
[leitura da prévia](D:/Nelcota/crates/admin/ui/src/lib/features/storage/preview.ts:2)
e [capacidade/limpeza do limitador](D:/Nelcota/crates/auth/src/rate_limit.rs:99).

### Comportamento e medição do limitador

Ao atingir a capacidade, **chaves novas recebem 429 com uma indicação de
nova tentativa em um segundo**. As chaves conhecidas conservam suas quotas
independentes. Nenhuma entrada recente é removida para dar uma nova quota
a alguém já bloqueado. A leitura de chaves existentes usa um lock
compartilhado; a admissão e a limpeza usam um lock exclusivo.

O [benchmark reproduzível](D:/Nelcota/crates/auth/benches/rate_limit.rs:1)
executa o limitador público real em build otimizada. Mediana de três lotes
de 100 tentativas com chaves novas: **18,7 µs** com 1.000 chaves e **6,3 µs**
com 50.000. O primeiro caso admitiu as 300 novas chaves; o segundo recusou
as 300 por capacidade. Essa medição verifica o novo caminho de admissão e
não representa throughput da API nem uma comparação com o antigo caso
que admitia chaves acima do limiar.

### Validação das correções da quarta revisão

| Verificação | Resultado |
|---|---|
| `cargo test --workspace -- --test-threads=4` | **346 passaram**, nenhum falhou; um teste S3 é ignorado por padrão e foi executado separadamente |
| S3: `cargo test -p nelcota-server --test storage_s3 -- --ignored` | Um passou com RustFS 1.0.1, incluindo expiração estrita e limite do TTL do presign |
| `cargo build -p nelcota-server --bin nelcota` | Binário atualizado compilou com os novos assets do painel |
| UI: `npm run check`, unitários e navegador | Nenhum erro/aviso; **215 unitários e 19 E2E passaram** |
| UI: `npm run build` | Build passou e atualizou os assets embarcados |
| SDK TypeScript: unitários, tipagem e contrato | **89 unitários, 11 de tipagem e 24 de contrato passaram**; contrato contra o binário atualizado, PostgreSQL 17 e Mailpit |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passou, incluindo os testes e o benchmark |
| `cargo fmt --all -- --check` e `git diff --check` | Passaram |
| Dependências | `cargo audit --quiet` passou; `npm audit --audit-level=low` encontrou zero vulnerabilidades na UI e no SDK |

Os testes desta rodada foram executados no Windows. Os cenários exclusivos
de Unix e um deployment Linux completo não foram repetidos. A exceção Rust
documentada para `RUSTSEC-2023-0071` continua restrita ao uso de verificação
pública RSA, sem as operações privadas afetadas.

Somando a suíte completa e o teste S3, **347 testes Rust distintos passaram**,
incluindo seis exemplos da documentação. O RustFS e os serviços temporários
de testes/contrato foram encerrados; permaneceram somente os três contêineres
que já estavam ativos. **Os quatro achados desta revisão estão corrigidos.**

## Quarta revisão — 10/10/2026

Foram confirmados **quatro novos problemas: três P2 e um P3**. Nenhum novo
P1 foi confirmado. Não encontrei uma nova falha de sanitização comprovada
nesta rodada. As correções dos achados 15–17 continuam presentes, com seus
testes de regressão. Esta seção registra a revisão; o código de produção
não foi alterado nesta rodada.

| ID | Prioridade | Área | Problema |
|---|---|---|---|
| 18 | P2 | Segurança | URLs assinadas continuam autorizando a leitura de arquivos privados por até aproximadamente 30 segundos após `exp` |
| 19 | P2 | Segurança/performance | Callbacks concorrentes reutilizam o mesmo estado OAuth para iniciar múltiplas trocas HTTP com o provedor, sem a quota aplicada aos outros endpoints de autenticação |
| 20 | P2 | Performance | A prévia do painel baixa o arquivo inteiro antes de aplicar seu orçamento de bytes; a listagem pode estar desatualizada após uma substituição |
| 21 | P3 | Performance | Atingir 50 mil chaves vivas no limitador dispara uma varredura completa em cada verificação; o limiar não é um limite de capacidade |

### 18. P2 — URL assinada aceita expiração vencida

**Locais:** [validação compartilhada de JWT](D:/Nelcota/crates/auth/src/keys.rs:61)
e [resolução do arquivo assinado](D:/Nelcota/crates/storage/src/signing.rs:81).
A validação compartilhada aceita 30 segundos de tolerância. A resolução
calcula o tempo restante com `saturating_sub`, mas não recusa zero: continua
buscando e entregando o arquivo privado.

**Reprodução:** com o router de produção, PostgreSQL 17 e storage em disco,
tokens corretamente assinados com `exp` vencido há 1, 10 e 29 segundos
retornaram `200` e os bytes privados; vencido há 31 segundos retornou `403`.
Uma URL efetivamente emitida pelo endpoint com `expires_in=1` também retornou
`200` depois de uma espera de 2 segundos. A primeira parte do probe usa a
chave do ambiente isolado somente para verificar as fronteiras de tempo;
a segunda demonstra o comportamento de uma URL real emitida pelo servidor.

**Impacto:** o período de acesso compartilhado ultrapassa a duração solicitada,
inclusive para URLs de duração muito curta. A tolerância de JWT da API está
documentada; aplicá-la a uma autorização de arquivo com prazo explícito
enfraquece esse prazo. Não é uma assinatura forjada nem acesso sem uma URL
originalmente autorizada. Em S3, o mesmo caminho pode chegar ao presign com
zero segundos restantes, que são elevados para um segundo; essa extensão
adicional é uma inferência do código, não uma reprodução S3 desta rodada.

**Correção recomendada:** exigir `exp > now` na resolução do grant, antes da
consulta do objeto e do presign. Conservar a tolerância prevista para JWTs
de API. Testar antes, no instante e depois da expiração, em disco e S3.

### 19. P2 — Um estado OAuth permite várias chamadas simultâneas

**Locais:** [callback público](D:/Nelcota/crates/auth/src/handlers.rs:233),
[leitura do estado pendente](D:/Nelcota/crates/auth/src/oauth/flow.rs:69)
e [troca HTTP após essa leitura](D:/Nelcota/crates/auth/src/oauth.rs:143).
`pending` apenas lê o estado. Ele continua disponível enquanto a troca HTTP
ocorre, sendo invalidado somente quando a primeira chamada conclui.
O handler de callback também não aplica a quota por IP usada por authorize,
token e verify.

**Reprodução:** um único `/authorize`, com a quota configurada em 1 por minuto,
forneceu um estado. Foram enviados 32 callbacks simultâneos com esse estado e
um código inválido. Um provedor local manteve as respostas pendentes até a
medição: **32 trocas HTTP começaram antes de qualquer uma terminar**. Após a
liberação, os callbacks retornaram o erro do provedor por redirecionamento.
Um callback posterior recebeu `400` sem iniciar outra troca, confirmando
que a amplificação ocorre enquanto o estado permanece pendente.

**Impacto:** um cliente sem sessão consegue multiplicar conexões e operações
externas a partir de um estado permitido, consumindo recursos da API e do
provedor. A duração das chamadas individuais já é limitada, mas sua
concorrência não é controlada por esse fluxo. Não foi demonstrado login
indevido ou emissão de múltiplas sessões a partir de um código PKCE.

**Correção recomendada:** reivindicar o estado atomicamente antes da chamada
externa, permitindo uma única troca em andamento por estado, sem manter uma
transação aberta durante a rede. Aplicar também um orçamento ao callback/
trocas em andamento. Testar duplicatas concorrentes, falha do provedor,
cancelamento e retomada por um novo authorize.

### 20. P2 — Prévia aplica o limite depois de bufferizar o arquivo

**Locais:** [leitura e verificação do Blob](D:/Nelcota/crates/admin/ui/src/lib/features/storage/components/FilePreviewDialog.svelte:43)
e [tipo de prévia baseado na listagem](D:/Nelcota/crates/admin/ui/src/lib/features/storage/files.ts:54).
O tamanho da listagem decide se a prévia pode abrir. O download por nome lê
a versão atual, que pode ter sido substituída; `response.blob()` então
consome todo o corpo antes da verificação de 1 MiB para texto ou 20 MiB
para mídia.

**Reprodução em Chromium:** a UI real foi executada pelo Vite, usando as
fixtures HTTP existentes. A listagem informou `readme.txt` como texto de
32 KiB e o endpoint de arquivo respondeu com 8 MiB de texto, simulando uma
substituição válida após a listagem. Instrumentação de `Response.blob`
registrou **8.388.608 bytes completamente lidos** antes de o diálogo mostrar
“A prévia não está disponível”. O orçamento de texto é 1.048.576 bytes.
Foi uma resposta simulada no navegador; a reprodução não usou um upload
real nessa etapa.

**Impacto:** o orçamento de prévia não limita a transferência nem o buffer.
Uma substituição legítima ou feita por alguém com permissão de upload pode
levar o painel a baixar arquivos muito maiores que o necessário. O custo
máximo depende dos limites de upload configurados; não foi observado nem
alegado um crash do navegador. Nenhum conteúdo ativo foi executado.

**Correção recomendada:** recusar antecipadamente um `Content-Length` maior
que o orçamento e ler o stream com contagem de bytes e cancelamento ao
atingir o limite, incluindo respostas sem tamanho declarado. Conservar o
download completo como ação explícita separada. Testar substituição após a
listagem, respostas chunked e fechamento do diálogo durante a leitura.

### 21. P3 — Limpeza do limitador fica no caminho de cada requisição

**Local:** [verificação do limitador](D:/Nelcota/crates/auth/src/rate_limit.rs:54).
Quando `inner.len() >= MAX_KEYS`, cada `check` executa `retain_recent`.
Enquanto as entradas são recentes, a varredura não reduz o conjunto abaixo
do limiar e a próxima chamada repete a limpeza. Novas chaves continuam
entrando: `MAX_KEYS` é um gatilho, não uma capacidade máxima.

**Reprodução:** uma cópia exata do módulo de produção, em build release e com
o relógio controlado já usado pelos seus testes, manteve as chaves recentes
com quota de 30 por minuto. O conjunto cresceu de 50.000 para **50.300 chaves**.
Mediana de três execuções de 100 verificações: **37 µs** com 1.000 chaves e
**6,912 ms** com 50.000, aproximadamente 187 vezes mais. O custo absoluto
nesse caso foi de cerca de 69 µs por verificação, por isso a prioridade é
baixa. A medição não representa throughput da API nem um ataque distribuído
efetivamente executado.

**Impacto:** com alta cardinalidade de IPs/e-mails, a manutenção passa a ter
custo linear síncrono por tentativa de autenticação. O limite por IP torna
esse cenário menos acessível a um cliente comum; o crescimento não implica
que entradas antigas sejam mantidas indefinidamente, pois a limpeza as
remove quando deixam de ser recentes.

**Correção recomendada:** agendar/amortizar a limpeza e definir uma política
de admissão ao atingir a capacidade, sem evictar entradas que ainda estão
bloqueando tentativas. Validar a capacidade e preservar o orçamento por chave
e a independência entre contas/IPs.

### Escopo e validação da quarta revisão

A revisão cobriu autenticação, sessões, recovery, vinculação de identidades,
OAuth e limitadores; construção de SQL, RPC, permissões e limites da API;
autenticação administrativa, CSRF, SSO, migrações, exportações e prévias do
painel; quotas, caminhos, MIME, URLs assinadas e limpeza do storage; CLI,
arquivos de configuração, backups, templates e atualização; transportes e
autenticação dos SDKs TypeScript e Rust; scripts, CI e dependências.

| Verificação executada nesta rodada | Resultado |
|---|---|
| `cargo test --workspace -- --test-threads=4` | **345 execuções passaram**, nenhuma falhou e uma foi ignorada: 338 testes permanentes, incluindo seis exemplos da documentação, mais sete execuções dos probes temporários (quatro testes copiados do limitador e três testes novos) |
| Probes de expiração e concorrência OAuth | Passaram usando o router de produção, PostgreSQL 17, storage em disco e um provedor HTTP local; confirmaram os achados 18 e 19 |
| Probe do limitador em release | Passou com o módulo de produção copiado e relógio controlado; confirmou crescimento e custo do achado 21 |
| Probe da prévia em Chromium | Confirmou o achado 20 na UI real, com resposta HTTP simulada de 8 MiB |
| UI: `npm run check`, unitários e E2E existentes | Nenhum erro ou aviso; **206 unitários e 15 E2E passaram** |
| SDK TypeScript: unitários e tipagem | **89 unitários e 11 testes de tipagem passaram** |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passou após a remoção dos probes temporários |
| `cargo fmt --all -- --check` e `git diff --check` | Passaram |
| Dependências | `cargo audit --quiet` passou; `npm audit --audit-level=low` encontrou zero vulnerabilidades na UI e no SDK |

O teste S3 ignorado e os 24 testes de contrato do SDK TypeScript passaram na
rodada anterior, mas não foram reexecutados nesta revisão. Os testes
exclusivos de Unix e um deployment Linux completo também não foram
repetidos nesta rodada, executada no Windows. A auditoria Rust conserva a
exceção documentada para `RUSTSEC-2023-0071`, referente a operações privadas
RSA que o projeto não utiliza.

Os testes que passam não anulam os quatro achados: os probes verificaram
explicitamente os comportamentos problemáticos. Os arquivos temporários
de reprodução foram removidos e os serviços criados para esta revisão
foram encerrados; ao final, permaneceram somente os três contêineres que
já estavam ativos. Na conclusão desta revisão, os achados 18–21 estavam
abertos; receberam depois as correções registradas no início deste relatório.

## Correções da terceira revisão — 10/10/2026

Os achados **15, 16 e 17 receberam correções e testes de regressão**.

| ID | Correção | Validação específica |
|---|---|---|
| 15 | Recovery usa a operação comum de primeira confirmação antes de instalar a senha nova; identidade/códigos/links anteriores são removidos sob o bloqueio da conta | Identidade e PKCE antigos recusados; refresh e magic link antigos inválidos; senha/refresh novos válidos; resgate concorrente recusado; provedor legítimo de conta já confirmada preservado |
| 16 | Cada linha do resumo exportado recebe seu próprio prefixo de comentário SQL, cobrindo LF, CR e CRLF | Sete nomes com controles, aspas e instruções SQL foram criados, exportados e recriados por refinery; nenhum registro foi inserido pela instrução embutida e o segundo migrate não repetiu a migração |
| 17 | A montagem do INSERT constrói um índice nome → posição uma vez por lote, agrupa por posições numéricas e resolve nomes de saída uma vez por grupo | Ordenação do catálogo/grupos preservada; valores, defaults e NULLs verificados em tabela real de 1.600 colunas; lotes inválidos e acesso sem permissão não publicam linhas |

Os novos testes de recovery e migrações falharam antes das respectivas
correções e passaram depois delas. Foram adicionados **oito testes permanentes**
(três unitários e cinco de integração), incluindo o cenário de concorrência.

Implementação: [recuperação](D:/Nelcota/crates/auth/src/recovery.rs:66),
[comentários das migrações](D:/Nelcota/crates/admin/src/migrations/files.rs:54)
e [montagem do INSERT](D:/Nelcota/crates/api/src/query/sql.rs:333).

### Validação final das correções

| Verificação | Resultado |
|---|---|
| `cargo test --workspace -- --test-threads=4` | 338 passaram, nenhum falhou; um teste S3 exige serviço externo e foi executado separadamente |
| S3: `cargo test -p nelcota-server --test storage_s3 -- --ignored` | Um passou com RustFS 1.0.1; somando a suíte completa, **339 testes Rust distintos passaram**, incluindo seis exemplos da documentação |
| `cargo build -p nelcota-server --bin nelcota` | Binário atualizado compilou |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passou, incluindo testes e benchmark |
| `cargo fmt --all -- --check` e `git diff --check` | Passaram |
| UI: `npm run check`, testes unitários e navegador | Nenhum erro/aviso; **206 unitários e 15 E2E passaram** |
| SDK TypeScript: testes unitários, tipagem e contrato | **89 unitários, 11 de tipagem e 24 de contrato passaram**; contrato executado contra o binário atualizado, PostgreSQL 17 e Mailpit |
| Auditorias de dependências | `cargo audit --quiet` passou; `npm audit --audit-level=low` encontrou zero vulnerabilidades na UI e no SDK |

A auditoria Rust conserva a exceção já documentada para
`RUSTSEC-2023-0071`: o projeto utiliza apenas verificação pública RSA, sem as
operações privadas afetadas. Os serviços temporários de teste foram removidos;
ao final, somente os três contêineres que já estavam ativos permaneceram.

### Medição do INSERT após a correção

O [benchmark reproduzível](D:/Nelcota/crates/api/benches/insert.rs:1) executa a
função de produção em build otimizada:
`cargo bench -p nelcota-api --bench insert`. A entrada é a mesma da revisão:
1.000 linhas, 100 chaves por linha, JSON de 1.002.001 bytes; construção/clonagem
da entrada fica fora da medição.

| Colunas do catálogo | Mediana anterior | Mediana corrigida |
|---|---|---|
| 100 | 39,03 ms | 3,51 ms |
| 400 | 136,78 ms | 3,53 ms |
| 800 | 276,88 ms | 3,86 ms |
| 1.600 | 504,62 ms | 3,60 ms |

No caso mais largo, a montagem ficou aproximadamente **140 vezes mais rápida**
na medição local. Esse ganho se refere à compilação do INSERT, não ao tempo
total da API ou à capacidade de produção. As varreduras repetidas do catálogo
foram removidas; a autorização continua sendo aplicada por PostgreSQL/RLS.

## Terceira revisão — 10/10/2026, achados anteriores à correção

Esta rodada encontrou **três problemas restantes: um P1 e dois P2**. O ID 15
era uma extensão do achado 11 em um fluxo que ainda não usava a proteção comum.
Os achados abaixo descrevem as reproduções **anteriores à correção**, preservadas
como histórico. As correções posteriores estão registradas no início do relatório.

| ID | Prioridade | Área | Problema encontrado |
|---|---|---|---|
| 15 | P1 | Segurança | Recuperação de senha confirma conta pendente sem remover identidades OAuth e códigos estabelecidos antes da confirmação |
| 16 | P2 | Sanitização | Quebras de linha em nomes aceitos pelo painel escapam dos comentários das migrações geradas |
| 17 | P2 | Performance | Preparação de INSERT repete buscas lineares por coluna e por conjunto em todas as linhas do lote, dentro da thread assíncrona |

### 15. P1 — Recuperação mantém meios de acesso anteriores à confirmação

**Local:** [recuperação de senha](D:/Nelcota/crates/auth/src/recovery.rs:64).
O fluxo revisado definia `email_confirmed_at`, trocava a senha e revogava sessões,
mas não chamava `confirm_inbox_owner` nem removia `auth.identities` e
`auth.flow_states` de uma conta que ainda não estava confirmada.

**Reprodução:** um provedor simulado informou um e-mail primário não verificado
para uma conta nova, comportamento aceito pelo backend. Foi aberta uma sessão
OAuth e deixado um segundo código PKCE pendente. O proprietário do endereço
solicitou `/auth/v1/recover`, recebeu o e-mail e concluiu
`/auth/v1/verify` com `type=recovery` e uma senha nova. O servidor confirmou o
e-mail e invalidou o refresh anterior (`400`), como esperado. Entretanto:

- O código OAuth anterior ao reset retornou `200` e uma nova sessão para o
  mesmo ID de usuário.
- Um novo login pela identidade não verificada anterior também retornou
  `200` para a conta já confirmada.
- O dono do endereço conseguiu entrar com sua senha nova: os dois meios de
  acesso coexistiram após a recuperação.

**Impacto:** o titular de uma identidade vinculada antes da prova de posse do
e-mail conserva acesso à conta mesmo após o proprietário recuperá-la. Revogar
apenas as sessões existentes não resolve, porque a identidade e o código
pendente emitem sessões novas. O cenário depende de OAuth habilitado, cadastro
aberto e de um provedor retornar um e-mail não verificado para uma conta nova;
não foi reproduzido contra contas reais de Google/GitHub.

**Correção recomendada:** sob o bloqueio da conta, tratar a primeira confirmação
por recovery com a mesma limpeza de credenciais anteriores usada por magic
link e OAuth verificado, antes de definir a senha escolhida pelo proprietário.
Preservar a senha nova, manter as identidades legítimas de contas que já estavam
confirmadas e conservar a ordem de bloqueios conta → credenciais/sessões.
Validar resgate concorrente de PKCE, login pela identidade antiga e renovação
das sessões antes/depois do reset.

**Evidência:** probe
`audit_round_three_recovery_retains_unverified_identity_and_pending_code`,
executada no router de produção, com PostgreSQL 17 descartável e endpoints
HTTP de um provedor simulado, passou reproduzindo os dois acessos indevidos.

### 16. P2 — Nomes com quebra de linha viram SQL fora do comentário

**Locais:** [renderização da migração](D:/Nelcota/crates/admin/src/migrations/files.rs:51)
e [validação de nomes](D:/Nelcota/crates/admin/src/ddl/mod.rs:84).
O painel permite quebras de linha internas em identificadores. A execução
imediata usa o identificador corretamente entre aspas, mas a migração insere
o resumo que contém esse nome em uma única linha iniciada por `--`, sem
proteger as linhas seguintes.

**Reprodução por HTTP autenticado do painel:**

1. Criar a tabela `orders\narchive` e gerar a migração retorna `200` em ambos
   os endpoints. Executar o SQL exportado em um contexto sem essa tabela
   retorna `42601`, `syntax error at or near "archive"`.
2. Criar a tabela com o nome
   `x\nSELECT set_config('app.audit','yes',false); --` também retorna `200`.
   A criação original trata tudo como nome de tabela. Executar a migração
   exportada executa um `SELECT` adicional: `current_setting('app.audit')`
   passa a ser `yes`, comprovado em PostgreSQL.

Nesses exemplos, `\n` representa uma quebra de linha real no nome enviado.
Os testes usaram somente bancos descartáveis; a instrução adicional alterou
apenas um parâmetro de sessão usado como marcador.

**Impacto:** migrações geradas a partir de entradas aceitas deixam de reproduzir
o schema ou executam instruções que não pertencem à alteração original.
Este caminho exige acesso administrativo ao painel; não demonstra escalada
de privilégio nem acesso público sem autenticação.

**Correção recomendada:** gerar o comentário prefixando **cada linha** do resumo
com `-- `, cobrindo LF, CR e CRLF; alternativamente, rejeitar controles nos
nomes criados/editados pelo painel, mantendo a proteção no exportador para
nomes preexistentes vindos do banco. Adicionar round trips criação → exportação
→ execução para nomes com aspas e quebras de linha, verificando que não surgem
instruções adicionais.

**Evidência:** probe
`audit_round_three_multiline_identifiers_escape_migration_comments`, com login
real do painel, seus endpoints de criação/exportação e PostgreSQL 17, passou
reproduzindo a migração inválida e a instrução adicional.

### 17. P2 — Lotes válidos bloqueiam a thread durante a montagem do INSERT

**Locais:** [agrupamento do INSERT](D:/Nelcota/crates/api/src/query/sql.rs:335),
[busca de coluna](D:/Nelcota/crates/api/src/catalog/model.rs:66) e
[preparação anterior à execução](D:/Nelcota/crates/api/src/operations.rs:186).
Para cada linha, `body_columns` busca cada chave linearmente nas colunas da
tabela. Em seguida, a ordenação percorre novamente todas as colunas e usa
`Vec::contains` sobre as chaves. Mesmo quando mil linhas têm o mesmo conjunto
de chaves, essas buscas são repetidas antes de consultar o índice de grupos.

**Medição da função de produção `query::insert`, em build `release` otimizada:**
lote com 1.000 linhas, 100 chaves por linha (`c1500` a `c1599`), valores inteiros
e **1.002.001 bytes** de JSON. Três execuções para cada tamanho de catálogo,
com construção/clonagem da entrada fora do intervalo medido:

| Colunas da tabela | Tempo das três execuções | Mediana |
|---|---|---|
| 100 | 39,52 / 35,77 / 39,03 ms | 39,03 ms |
| 400 | 133,06 / 136,78 / 136,87 ms | 136,78 ms |
| 800 | 277,19 / 276,88 / 269,81 ms | 276,88 ms |
| 1.600 | 496,64 / 504,62 / 507,55 ms | 504,62 ms |

A requisição está abaixo do limite de corpo de 2 MiB, respeita as 1.000 linhas
e usa apenas **um** conjunto de colunas. A probe HTTP criou uma tabela real de
1.600 colunas, sem GRANT de INSERT para `anon`. Um POST sem JWT percorreu a
preparação e só depois recebeu `401` de PostgreSQL por falta de permissão.
Esse teste comprova que o processamento é alcançável antes da negativa de
autorização; os tempos da tabela acima vêm da compilação otimizada da função,
não do teste HTTP em debug nem de um benchmark de produção completo.

**Impacto:** em tabelas largas, uma requisição válida em tamanho ocupa a thread
do executor assíncrono por centenas de milissegundos antes do primeiro acesso
ao banco. Requisições simultâneas competem com login, consultas e health checks
pelas mesmas threads. Os limites recentes de linhas/grupos impedem lotes
ilimitados, mas não eliminam essas varreduras repetidas.

**Correção recomendada:** construir uma busca por nome → posição/coluna uma vez
por catálogo/tabela ou compilação; ordenar e validar cada conjunto distinto
uma única vez, preservando a ordem original de agrupamento e os defaults.
Caso reste trabalho relevante de CPU, executá-lo fora das threads assíncronas
com concorrência limitada. Validar a equivalência dos SQLs, rejeição de colunas
desconhecidas, lotes heterogêneos e latência de outras requisições sob carga.

**Evidência:** probes `audit_round_three_insert_compiler_wide_table_cost`
(`cargo test -p nelcota-api --release --test reaudit_round_three`) e
`audit_round_three_unauthorized_public_insert_compiles_the_wide_batch`.

### Validação e limites desta rodada

- `cargo audit --quiet`: concluiu com sucesso, sem vulnerabilidades não
  ignoradas. A exceção RSA previamente documentada permanece explícita.
- `npm audit --audit-level=low` na UI e no SDK TypeScript: zero vulnerabilidades.
- `cargo test --workspace --lib --quiet`: **168 testes aprovados**.
- Regressões de integração: **42 testes aprovados** — auth (22), OAuth (12),
  concorrência (6), CSV e plano da listagem (2). A probe do ID 15 também foi
  executada novamente junto à suíte OAuth e reproduziu a falha outra vez.
- UI: **206 testes aprovados**; `npm run check` sem erros nem avisos Svelte/TS.
- SDK TypeScript: **89 testes unitários** e **11 testes de tipos** aprovados.
- `cargo fmt --all -- --check` e `git diff --check`: aprovados após remover
  as probes temporárias. Ao final, somente os três containers de desenvolvimento
  já existentes permaneceram ativos.
- As quatro probes acima reproduziram os achados em isolamento. São testes
  de diagnóstico que confirmam problemas existentes, não validação de correções.
  Seus arquivos/blocos temporários foram removidos depois da execução; os
  testes existentes e o código de produção foram preservados.

A revisão cruzou autenticação, sessões, JWT/OAuth, RLS, compilação/execução de
SQL e limites de resposta, armazenamento, uploads, URLs assinadas, painel,
exportadores, SDKs, CLI, backups, configuração de implantação e workflows.
Não é uma garantia de ausência de outros problemas ou um teste de carga do
ambiente de produção. O principal ponto de segurança restante é o ID 15.

## Nova revisão — 10/10/2026

A nova revisão identificou **quatro problemas adicionais**: um de prioridade
alta e três de prioridade média. Todos receberam correções. A revisão cobriu
autenticação/JWT/OAuth e concorrência, RLS e SQL dinâmico, storage e seus limites,
painel Svelte, SDKs TypeScript/Rust, CLI, backups e workflows. Os achados foram
reproduzidos em PostgreSQL 17 descartável e em container Linux; os bancos de
desenvolvimento existentes não foram usados.

| ID | Prioridade | Problema | Correção |
|---|---|---|---|
| 11 | P1 | Conta confirmada herda credenciais anteriores à prova de propriedade do e-mail | Ao confirmar conta pendente por magic link/OAuth verificado, remove senha e identidades anteriores e revoga sessões e códigos/links pendentes, na mesma transação |
| 12 | P2 | Exportação CSV mantém fórmulas de dados não confiáveis | Exportadores do servidor e navegador neutralizam células e cabeçalhos perigosos; JSON conserva os dados originais |
| 13 | P2 | Raiz do storage em disco é legível por outros usuários Unix | CLI cria a raiz com 0700; servidor impõe 0700 antes de abrir diretórios novos e existentes |
| 14 | P2 | Listagem materializa todos os metadados antes de paginar | CTE `NOT MATERIALIZED` permite projetar apenas nomes para pastas e usar o índice para a página de objetos |

### 11. P1 — Credenciais anteriores à prova de propriedade do e-mail são mantidas

**Locais atuais:** [fluxo de magic link](D:/Nelcota/crates/auth/src/links/sign_in.rs:35),
[confirmação da propriedade da conta](D:/Nelcota/crates/auth/src/accounts.rs:23).

**Reprodução anterior à correção:** com confirmação obrigatória, um visitante
cadastrou o e-mail da vítima com uma senha conhecida. Login com essa senha
retornou `400 email_not_confirmed`. A vítima solicitou e abriu seu próprio
magic link. Em seguida, a mesma senha retornou `200`, para o mesmo ID de conta.
O atacante não precisava ler o e-mail nem obter o link.

**Impacto:** permite acesso à conta depois que o proprietário prova a posse do
e-mail por um fluxo independente do cadastro que definiu a senha.

A revisão dos outros meios de acesso confirmou uma extensão do mesmo problema:
quando o provedor retorna um e-mail não verificado, uma conta nova pode receber
uma identidade OAuth antes da confirmação. Essa identidade e um código PKCE
pendente continuavam abrindo sessões após uma confirmação por magic link ou
por outra identidade verificada. Dois testes com PostgreSQL e endpoints HTTP
de provedor simulado reproduziram o resgate indevido antes da correção; não
foram usadas contas reais de Google/GitHub.

**Correção:** magic link e OAuth verificado agora compartilham a mesma operação
para reivindicar contas pendentes. Sob o bloqueio da conta, a operação remove
`encrypted_password`, confirma o e-mail, revoga sessões anteriores e apaga
identidades e códigos/links pendentes, antes de emitir a nova sessão. No OAuth,
a identidade verificada é inserida após essa limpeza. Login por senha revalida o hash sob o
mesmo bloqueio, impedindo que um hash verificado antes da mudança crie sessão
depois dela. Contas já confirmadas mantêm suas credenciais. A confirmação
normal do cadastro continua aprovando sua senha.

A consulta da identidade e o resgate PKCE passam a bloquear a conta antes da
identidade/código e revalidam a credencial depois da espera. Isso impede que um
callback ou resgate concorrente recrie o acesso revogado e evita inverter a
ordem de bloqueios em relação à confirmação por e-mail.

**Testes:** [cenários de magic link](D:/Nelcota/crates/server/tests/auth.rs:679)
verificam a eliminação da senha pendente e da confirmação antiga, revogação de
refresh e recuperação anteriores, funcionamento da sessão recém-criada e
preservação de senha/sessões de uma conta já confirmada. A suíte também mantém
a regressão de confirmação normal de cadastro. Os
[testes OAuth](D:/Nelcota/crates/server/tests/oauth.rs:487) cobrem descarte de
identidades/códigos antigos, preservação da identidade verificada atual e
resgate PKCE concorrente com a confirmação: apenas a sessão do proprietário
permanece ativa, sem deadlock.

**Comportamento documentado:** o proprietário de uma conta pendente que entra
por magic link pode definir uma nova senha por recuperação. JWTs de acesso
anteriores mantêm sua expiração normal, seguindo o contrato existente de
logout/recuperação; a revogação impede novos refreshes.

### 12. P2 — Fórmulas em CSV provenientes de entradas públicas

**Locais atuais:** [CSV do servidor](D:/Nelcota/crates/admin/src/tables/export.rs:112),
[CSV do navegador](D:/Nelcota/crates/admin/ui/src/lib/download.ts:6).

**Reprodução anterior à correção:** um visitante autorizado a inserir linhas
gravou `=1+1`, `+1+1` e `@SUM(1,1)`. A exportação autenticada do painel produziu
`1,=1+1`, `2,+1+1` e `3,"@SUM(1,1)"`. Aspas RFC 4180 não impedem interpretação
como fórmula quando o administrador abre esses dados em uma planilha.
O risco depende da abertura do CSV em um leitor que execute fórmulas.

**Correção:** ambos os exportadores acrescentam um tab dentro de campo entre
aspas para prefixos perigosos, incluindo `=`, `+`, `-`, `@`, variantes fullwidth,
controles e fórmulas precedidas por espaços/BOM. Aspas internas continuam
duplicadas; cabeçalhos também recebem proteção. Números negativos que são
literais JSON válidos conservam seus dígitos sem conversão para `f64`.
Esse tratamento segue a proteção para Excel descrita pela
[OWASP — CSV Injection](https://community.owasp.org/attacks/CSV_Injection).

**Testes:** dois testes Rust do encoder, 17 novos cenários no navegador e
[integração da exportação](D:/Nelcota/crates/server/tests/reaudit.rs:10)
verificam fórmulas, controles, cabeçalhos, aspas, números de alta precisão e
JSON sem alteração. O teste HTTP insere as entradas pela API pública e
exporta pelo painel. Foi validado o arquivo produzido, sem executar Excel.

**Compatibilidade:** CSV acrescenta o prefixo às células perigosas. JSON é a
opção para exportar seu texto original. O comportamento de importação pode
variar entre leitores de planilhas; a neutralização não altera os dados no banco.

### 13. P2 — Arquivos privados acessíveis a outros usuários do host

**Locais atuais:** [abertura do disco](D:/Nelcota/crates/storage/src/store.rs:150),
[raiz privada](D:/Nelcota/crates/storage/src/private_dir.rs:6),
[scaffold Docker](D:/Nelcota/crates/cli/src/scaffold.rs:204).

**Reprodução:** em Linux com umask 022, `object_store 0.14.2` criou diretórios
0755 e uploads 0644. Outro UID, sem autenticação na API, conseguiu ler um
arquivo da raiz antiga. A raiz criada pelo CLI não tinha modo privado
explícito; o bind mount Docker conserva as permissões do host.

**Impacto:** usuários locais com permissão de percorrer os diretórios podiam
enumerar e ler uploads privados diretamente, ignorando as políticas RLS.

**Correção:** a raiz é criada em 0700 antes de qualquer upload; raízes já
existentes também recebem 0700 ao abrir o storage. O CLI mantém o proprietário
65532 usado pela imagem Docker. O bloqueio de leitura e percurso na raiz
protege todos os descendentes, inclusive arquivos antigos e temporários,
sem percorrer a árvore inteira em cada inicialização.

**Testes:** [teste Unix do store](D:/Nelcota/crates/storage/src/store.rs:37)
cobre raízes novas/existentes e uploads simples/multipart; o helper do CLI
testa criação e reabertura. Um container Linux executou os helpers reais e
a versão real de `object_store`, com umask 022: o UID `nobody` leu a raiz
anterior e foi bloqueado em **seis tentativas** na configuração corrigida
(simples/multipart, raiz nova, existente e criada pelo helper do scaffold).
Os arquivos continuaram legíveis pelo proprietário. A prova não exigiu um
deploy completo do servidor em Linux.

**Compatibilidade:** o processo precisa ser proprietário da raiz e poder
alterar seu modo. A proteção Unix não configura ACLs no Windows; esse requisito
está explicitado em [storage.md](D:/Nelcota/docs/storage.md).

### 14. P2 — Paginação do storage copia metadados de todo o bucket

**Local atual:** [SQL de listagem](D:/Nelcota/crates/storage/src/list.sql:2).

**Reprodução:** a CTE `under` era referenciada duas vezes, fazendo PostgreSQL
materializar todas as colunas, incluindo metadata JSON, antes de aplicar o
`LIMIT` de objetos. No fixture de 100.000 arquivos, metadata de 1.024 caracteres
e página de dez objetos, foram escritos **14.441 blocos temporários de 8 KiB**,
aproximadamente **112,8 MiB**, para cada consulta. Isso consome I/O e conexões
mesmo quando a resposta tem apenas dez linhas.

**Correção:** `NOT MATERIALIZED` permite ao planejador separar a projeção dos
nomes de pastas da busca da página de objetos. O SQL está em um arquivo usado
tanto pela operação real quanto pelo teste de plano. Essa opção segue o
comportamento documentado de
[CTEs no PostgreSQL 17](https://www.postgresql.org/docs/17/queries-with.html#QUERIES-WITH-CTE-MATERIALIZATION).
Políticas RLS, escapes de prefixo e paginação independente de pastas/objetos
continuam na mesma consulta e no mesmo snapshot.

**Medição após a correção:** no mesmo fixture, três execuções por versão:

| Versão | Tempos (ms) | Mediana | Blocos temporários escritos |
|---|---|---|---|
| Anterior | 404,460 / 345,766 / 338,313 | 345,766 ms | 14.441 |
| Corrigida | 46,737 / 43,513 / 25,448 | 43,513 ms | 0 |

A mediana foi aproximadamente **7,9 vezes menor** nessa execução local.
Não é uma promessa de throughput de produção; outras tarefas de validação
estavam ativas durante a medição. Pastas ainda exigem examinar os nomes
visíveis para deduplicação, e paginação por offset continua com seu custo normal.

**Regressão:** [teste de plano e página real](D:/Nelcota/crates/server/tests/reaudit.rs:68)
usa 10.000 objetos com metadata de 2 KiB e work_mem de 1 MiB; exige zero blocos
temporários escritos e valida os objetos/metadata retornados com offset.
Os testes existentes cobrem RLS, pastas, prefixos escapados e páginas de pastas.

### Validação da nova revisão

Testes direcionados de autenticação e os dois novos testes HTTP/plano passaram.
Painel: **206 testes unitários** e **15 E2E** passaram, com checagem
Svelte/TypeScript sem erros ou avisos e build atualizado. SDK TypeScript:
**89 testes unitários** e **11 testes de tipos** passaram. `cargo audit`
e `npm audit` do painel/SDK não encontraram vulnerabilidades não ignoradas;
a exceção RSA existente e documentada foi mantida.

`cargo test --workspace -- --test-threads=3` terminou com **327 testes passando**
e um S3 ignorado por depender de serviço externo. Após ampliar a limpeza de
credenciais para identidades/códigos OAuth, foram executados novamente **22
testes de autenticação, 12 OAuth, 6 de concorrência e 6 do SDK Rust**; todos
passaram, incluindo três cenários OAuth novos. São **330 testes Rust distintos**,
sem contar repetições. O teste S3 foi executado separadamente contra RustFS real
e passou: **331 testes Rust distintos no total**.

Os **24 contratos do SDK TypeScript** passaram contra o binário reconstruído,
PostgreSQL e Mailpit reais, também depois da extensão da correção OAuth.
Clippy em todos os targets com warnings tratados como erros, formatação,
build do binário/painel e `git diff --check` passaram. Nenhum dado de produção
foi alterado. O teste temporário de benchmark foi removido; os testes de
regressão permanentes permanecem no projeto.

A pasta ignorada `target/reaudit-linux-storage`, contendo a prova Linux e seus
artefatos de compilação, permanece no workspace. A política automática de
execução bloqueou sua remoção (`blocked by policy`, sem justificativa adicional),
inclusive após verificar o caminho absoluto e usar alvo literal. Os containers
descartáveis desta revisão foram encerrados; somente os containers que já
existiam permaneceram ativos.

## Correções da auditoria inicial — 09/10/2026

Os dez achados abaixo receberam correções no código. A evidência da auditoria
inicial foi preservada nas seções seguintes para permitir comparação; as
descrições de vulnerabilidade nessas seções correspondem ao estado anterior.

| ID | Correção aplicada |
|---|---|
| 1 | Leituras e RPCs de conjunto usam teto padrão de 1.000 linhas, aplicado também às relações; representações de escrita limitam relações a 1.000. Projeções duplicadas são rejeitadas. A migração V11 impõe orçamento de JSON de 8 MiB antes da agregação no Postgres, compartilhado com valores intermediários. Estouro retorna 413 e desfaz a escrita. Exportações administrativas continuam em streaming. |
| 2 | SQL dinâmico usa preparação sem cache. Apenas operações de contexto com SQL fixo continuam em cache. |
| 3 | Inserções aceitam até 1.000 linhas e 128 conjuntos distintos de colunas; agrupamento usa HashMap e preserva os defaults. Seleções aceitam até 128 itens no total, incluindo relações aninhadas. |
| 4 | Login e administração de contas do painel usam o mesmo Arc<Passwords> e semáforo da autenticação pública. O limite de tentativas do painel agora é por IP, com a mesma política de confiança no proxy. |
| 5 | O span HTTP registra método e caminho; a query string não entra no log, inclusive em TRACE. |
| 6 | Até 32 uploads ativos, com recusa imediata por 503 quando ocupados. Admissão, consultas preliminares, espera de cota e recepção compartilham um prazo. A publicação conserva o fechamento transacional para evitar remover bytes depois de um commit ambíguo. |
| 7 | Criação de arquivos privados em modo 0600, antes da escrita; diretórios de backup em 0700. Cópias, arquivos existentes, manifests, archives e downloads seguem a política. O transporte AWS usa umask 077. |
| 8 | Validação comum de endereço simples, compatível com lettre::Address: rejeita controles, display names, segmentos vazios e domínios malformados antes do hash e do SQL. Preserva trim/lowercase; não altera contas existentes automaticamente. |
| 9 | A migração V12 inicializa storage.usage com escritores bloqueados. Triggers por statement mantêm o total em INSERT, UPDATE, DELETE, UPSERT e TRUNCATE, inclusive SQL administrativo e rollback; lotes agregam suas alterações uma vez. A cota lê uma linha. Reconciliação de manutenção documentada. |
| 10 | Shell e páginas privadas carregam dinamicamente após a sessão. JavaScript inicial: 930.513 → 396.634 bytes (redução de 57,4%); gzip: 242.268 → 103.516 bytes. Estados de espera e erro permitem acompanhar ou repetir o carregamento. |

As mudanças incluem [V11](D:/Nelcota/migrations/V11__bounded_api_json.sql),
[V12](D:/Nelcota/migrations/V12__storage_usage.sql),
[testes de regressão](D:/Nelcota/crates/server/tests/hardening.rs) e
[teste do carregamento após login](D:/Nelcota/crates/admin/ui/e2e/lazy-login.spec.ts).

**Limites relevantes:** o orçamento de JSON é conservador porque contabiliza
valores intermediários de relações. Um valor individual precisa ser serializado
antes da checagem; ele não pode entrar em uma agregação acima do orçamento.
RPCs continuam sendo funções SQL definidas pelo projeto, sujeitas às permissões
e ao statement_timeout; o teto de resposta não transforma essas funções em um
sandbox de execução. O cap de inserção exige dividir lotes maiores. Modos Unix
não substituem ACLs no Windows. Validação dos helpers e do transporte em Linux
não equivale a um deploy completo com systemd.

### Validação das correções

- `cargo test --workspace -- --test-threads=3`: **320 testes passaram**;
  o teste S3 ignorado nessa execução foi executado separadamente contra RustFS,
  com sucesso (**321 testes Rust no total**).
- `cargo clippy --workspace --all-targets -- -D warnings`, formatação, build
  do binário e verificação de whitespace: passaram.
- Painel: **189 testes unitários**, checagem Svelte/TypeScript sem erros ou
  avisos e build do dist versionado. **15 E2E com 3 workers** passaram, incluindo
  login por link interno, edição, uploads, migrações e SQL.
- SDK TypeScript: **89 testes unitários e 24 contratos** contra binário,
  PostgreSQL e Mailpit reais passaram.
- Linux: helpers Rust verificaram modos 0600/0700 com umask 022, incluindo
  criação antes da escrita, sobrescrita, cópia e fechamento das permissões.
  Transporte AWS real verificou download com arquivo 0600 e diretório 0700.
- `cargo audit`: zero vulnerabilidades não ignoradas; a exceção RSA já
  documentada permanece. `npm audit` do painel e SDK: zero vulnerabilidades.

Os E2E inicialmente tiveram timeouts em `networkidle`, com páginas já prontas
e requisições de módulos Vite pendentes. A espera foi substituída por conclusão
das respostas esperadas da API e remoção dos skeletons, mantendo a checagem dos
erros de contrato. Essa forma de avaliar prontidão segue a
[documentação do Playwright](https://playwright.dev/docs/api/class-page#page-wait-for-load-state).
Não foi necessário aumentar timeouts ou reduzir o paralelismo.

## Resumo

Foram identificados **10 achados: 2 de prioridade alta (P1), 5 de prioridade média (P2) e 3 de prioridade baixa (P3)**. Os maiores riscos são consumo de memória sem limite efetivo em respostas da API e retenção indefinida de consultas preparadas controladas pelo cliente. Há também exposição de tokens em logs DEBUG, concorrência excessiva no login do painel, espera de uploads sem prazo e permissões insuficientes para backups em Unix.

As reproduções confirmaram os comportamentos descritos abaixo. Os riscos de esgotamento de memória são consequências possíveis dos limites ausentes; não foi provocado OOM. O problema de permissões de backup foi confirmado por inspeção e pela especificação da biblioteca padrão, sem executar o CLI em um host Linux real nesta etapa.

| ID | Prioridade | Área | Achado |
|---|---|---|---|
| 1 | P1 | Performance / disponibilidade | Limite de linhas não controla embeds, RPC ou tamanho da resposta |
| 2 | P1 | Performance / disponibilidade | Cache de SQL dinâmico cresce sem descarte |
| 3 | P2 | Sanitização / disponibilidade | Lotes JSON permitem complexidade e expansão excessivas de SQL |
| 4 | P2 | Segurança / disponibilidade | Login do painel contorna o orçamento de Argon2 e compartilha um limite global |
| 5 | P2 | Segurança | Tokens de URLs assinadas aparecem em logs DEBUG |
| 6 | P2 | Performance / disponibilidade | Fila de uploads fica fora do timeout |
| 7 | P2 | Segurança | Backups não são criados com permissões privadas explícitas |
| 8 | P3 | Sanitização | Validação de email aceita endereços malformados e caracteres de controle |
| 9 | P3 | Performance | Cota de storage exige duas somas completas por publicação |
| 10 | P3 | Performance | Login carrega módulos de funcionalidades privadas antecipadamente |

## Escopo e método

Revisados os caminhos de autenticação, JWT, OAuth, autorização por RLS, geração e execução de SQL, storage em disco/S3, painel Svelte, transportes dos SDKs TypeScript/Rust, configuração, backups e workflows de dependências/publicação. A skill `security-best-practices` orientou a parte JavaScript/TypeScript; ela não contém referência específica para Rust/Axum ou Svelte. O backend foi examinado diretamente, com consulta a documentação primária quando necessário.

As reproduções utilizaram `TestApp` com **PostgreSQL 17 descartável**, pool de uma conexão e `max_rows=1`. Não foram executadas contra os bancos de desenvolvimento existentes. O teste temporário foi removido após registrar os resultados. Os tempos do compilador de consultas foram medidos em **build Rust de testes, sem otimização, no Windows**; demonstram o comportamento e sua escala, não throughput de produção.

A etapa inicial produziu documentação e reproduções. As correções posteriormente
autorizadas estão registradas no início deste relatório. As referências de linha
e medições das seções de achados são as da auditoria de 09/10.

## Prioridade alta

### 1. P1 — Respostas podem superar amplamente o orçamento de leitura

**Locais:** [SQL dos embeds](D:/Nelcota/crates/api/src/query/sql.rs:70), [limite da consulta principal](D:/Nelcota/crates/api/src/query/sql.rs:211), [SQL de RPC](D:/Nelcota/crates/api/src/query/sql.rs:470), [buffer da resposta](D:/Nelcota/crates/api/src/operations.rs:57), [seleção de colunas](D:/Nelcota/crates/api/src/query/parse.rs:44).

`max_rows` limita somente a consulta principal. Os arrays de relações usam apenas o `limit` enviado pelo cliente, e RPCs que retornam conjuntos usam `json_agg` sem esse limite. Além disso, `select` aceita repetir a mesma coluna. PostgreSQL monta o JSON completo e o servidor recebe um `String` completo antes de responder. O valor padrão de `max_rows` é `None`, em [config.rs](D:/Nelcota/crates/core/src/config.rs:194).

**Reprodução:** com `max_rows=1`, uma tabela de pais e 2.000 filhos contendo texto de 1.024 bytes:

- `GET /rest/v1/audit_parents?select=id,audit_children(id,payload)` retornou **1 pai, 2.000 filhos e 2.102.919 bytes**.
- `POST /rest/v1/rpc/audit_many` retornou **2.000 registros e 2.130.891 bytes**.
- Repetir `payload` 100 vezes em `select`, com `limit=1`, gerou **103.703 bytes** a partir de um campo de 1.024 bytes. O JSON bruto contém chaves repetidas, embora parsers normalmente conservem apenas a última.

**Impacto:** um cliente com acesso a dados volumosos pode consumir muita memória no banco e no servidor, comprometendo a disponibilidade. É necessário ter `SELECT`/`EXECUTE` e acesso pelas políticas; `anon` também pode alcançar isso quando o projeto abre esses dados. Os timeouts existentes limitam duração, mas não os bytes de uma resposta construída rapidamente.

**Correção:** definir um teto padrão, aplicá-lo também a embeds e RPCs de conjunto, rejeitar/deduplicar projeções repetidas e estabelecer orçamento de resposta. Evitar agregar toda a resposta em uma única célula PostgreSQL quando o volume exige streaming. Um limite aplicado somente após `query_one` não impede a alocação anterior no banco.

**Mitigação atual:** configurar `NELCOTA_MAX_ROWS`, paginar relações explicitamente e limitar as funções expostas. Isso reduz parte do risco; o teto atual continua podendo ser superado por relações, RPC e largura das linhas.

### 2. P1 — Consultas controladas pelo cliente permanecem no cache sem limite

**Locais:** [leitura](D:/Nelcota/crates/api/src/operations.rs:56), [escrita](D:/Nelcota/crates/api/src/operations.rs:62), [RPC](D:/Nelcota/crates/api/src/operations.rs:274), [criação do pool](D:/Nelcota/crates/core/src/db.rs:111).

Os caminhos públicos usam `prepare_cached` para SQL cuja estrutura depende de `select`, filtros, ordenação, embeds e lotes. O cache de `deadpool-postgres` mantém as consultas por conexão. Não há descarte por quantidade, tamanho ou idade no projeto. O código da dependência resolvida, versão 0.14.2, usa um mapa sem política de expulsão; a API oferece limpeza explícita, não um teto automático. [Documentação do cache](https://docs.rs/deadpool-postgres/latest/deadpool_postgres/struct.StatementCache.html).

**Reprodução:** 150 requisições aceitas, variando o número de colunas `id` em `select`, fizeram o cache de uma única conexão passar de **4 para 154 entradas**. A conexão PostgreSQL passou a ter **155 prepared statements**. A liberação da conexão para o pool não removeu as consultas.

**Impacto:** pedidos pequenos e sequenciais podem acumular memória na aplicação e no PostgreSQL durante toda a vida das conexões. Parametrizar os valores protege contra injeção, mas não limita o número de formatos diferentes de consulta. A rejeição de colunas repetidas corrige essa reprodução específica; outras combinações de filtros, ordens e aliases continuam produzindo formatos distintos.

**Correção:** usar preparação sem cache para SQL ad hoc, mantendo cache para os comandos fixos, ou implementar um cache com limite por conexão e descarte efetivo dos statements. Medir quantidade e bytes de SQL retidos. Considerar também as tentativas de preparação que falham, para que não acumulem entradas auxiliares.

**Mitigação atual:** reiniciar ou reciclar conexões libera o estado, mas não constitui um limite permanente.

## Prioridade média

### 3. P2 — O limite em bytes do JSON não limita o custo dos lotes

**Locais:** [agrupamento dos objetos](D:/Nelcota/crates/api/src/query/sql.rs:305), [busca linear do grupo](D:/Nelcota/crates/api/src/query/sql.rs:314), [objetos vazios](D:/Nelcota/crates/api/src/query/sql.rs:324), [CTEs](D:/Nelcota/crates/api/src/query/sql.rs:346).

O agrupamento por conjunto de colunas busca linearmente os grupos existentes. Com muitos conjuntos distintos, o trabalho cresce de forma quadrática. Para objetos vazios, cada `{}` gera um `INSERT DEFAULT VALUES` e uma CTE própria. Não há teto de registros, grupos ou complexidade do SQL. A construção ocorre de forma síncrona antes de aguardar o banco.

**Reprodução:**

| Entrada | JSON | SQL produzido | Tempo de construção |
|---|---:|---:|---:|
| 80.000 objetos vazios | 240.001 bytes | 5.828.906 bytes | 46 ms |
| 4.000 conjuntos distintos, tabela com 15 colunas | 178.002 bytes | 830.277 bytes | 54 ms |
| 16.000 conjuntos distintos | 839.618 bytes | 3.563.142 bytes | 810 ms |
| 28.000 conjuntos distintos | 1.536.434 bytes | 6.369.574 bytes | 2.585 ms |

As entradas estão abaixo do limite HTTP padrão de 2 MiB. As maiores foram somente compiladas, sem executar a mutação. Um lote de 2.000 objetos vazios foi executado no banco isolado e aceito com HTTP 201.

**Impacto:** clientes com permissão de inserção podem causar trabalho desproporcional de CPU, memória e planejamento SQL. O timeout assíncrono não consegue interromper imediatamente a etapa síncrona de construção.

**Correção:** validar quantidade de registros, grupos distintos e projeções antes de gerar SQL; agrupar por uma chave em mapa, em vez de busca linear; limitar a expansão de objetos vazios. Rejeitar lotes acima do orçamento com erro claro. Qualquer divisão interna deve preservar a atomicidade prometida pela API.

### 4. P2 — Login público do painel não participa do orçamento de Argon2

**Locais:** [limite global](D:/Nelcota/crates/admin/src/auth.rs:112), [verificação sem semáforo](D:/Nelcota/crates/admin/src/auth.rs:125), [quota de dez tentativas](D:/Nelcota/crates/server/src/main.rs:151), [limite aplicado à auth dos usuários](D:/Nelcota/crates/auth/src/password.rs:41).

O login do painel chama diretamente `spawn_blocking(verify_password)`. A auth dos usuários usa `Passwords` com semáforo e mantém a vaga até o worker terminar, inclusive após cancelamento. O painel não usa esse mecanismo. O limite de dez tentativas por minuto permite um burst inicial de dez; não é um limite de concorrência. O email errado também executa a verificação, corretamente para evitar diferença de tempo, mas sem controlar o custo.

**Reprodução:** dez requisições simultâneas ao login, com credenciais inválidas, executaram e retornaram 401. O hash padrão usa `m=19456`, aproximadamente **19 MiB por operação**; dez verificações podem reservar cerca de **190 MiB somente para Argon2**, além das operações de auth normais. Não foi provocado OOM nem usado um container de 128 MiB para essa medição.

A chave literal `admin-login` também compartilha o orçamento entre todos os clientes. Um atacante sem credenciais pode consumir as dez tentativas iniciais e depois cada reposição de orçamento, impedindo o administrador de iniciar sessão. Isso exige somente que o painel esteja habilitado e acessível. O refill de dez/minuto disponibiliza uma tentativa a cada seis segundos.

**Correção:** compartilhar um orçamento de hashing limitado entre os caminhos de autenticação; adquirir a vaga antes de `spawn_blocking` e mantê-la no worker. Combinar limite por origem confiável com proteção contra ataques distribuídos, sem permitir que uma única origem esgote continuamente todo o acesso legítimo ao painel. Revisar também o hashing nas operações administrativas de usuários.

### 5. P2 — Logs DEBUG incluem credenciais presentes na query string

**Locais:** [TraceLayer padrão](D:/Nelcota/crates/server/src/lib.rs:125), [filtro configurável por ambiente](D:/Nelcota/crates/server/src/main.rs:246), [URL assinada com token](D:/Nelcota/crates/storage/src/http.rs:209).

O span padrão do `tower-http` inclui a URI completa, com query string. O fato de não registrar headers protege `Authorization`, mas não `?token=...`. A configuração normal é INFO; o problema aparece ao habilitar DEBUG para `tower_http`, por exemplo durante diagnóstico. [Implementação de DefaultMakeSpan](https://docs.rs/tower-http/latest/src/tower_http/trace/make_span.rs.html).

**Reprodução:** foi criado um arquivo privado no storage do teste, emitida uma URL assinada por 300 segundos e realizado o download sem bearer. Com `warn,tower_http=debug`, o log capturado continha **o token válido completo**. Não foram salvos tokens reais neste relatório.

**Impacto:** quem obtiver esses logs pode reutilizar a URL para baixar o arquivo enquanto ela for válida. Callbacks OAuth também contêm `code` e `state`; PKCE continua oferecendo sua proteção, mas esses parâmetros igualmente não devem ser registrados sem necessidade. A exclusão da query string em `DeniedLog` não cobre o TraceLayer externo.

**Correção:** criar um span próprio que registre `request.uri().path()` e omita a query, ou redija explicitamente todos os parâmetros sensíveis. A redação precisa funcionar em todos os níveis de log. [Orientação de logging da OWASP](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html).

**Mitigação atual:** manter o filtro normal INFO até corrigir a redação. Não há evidência de vazamento de tokens pelo TraceLayer com o filtro padrão.

### 6. P2 — Espera por vaga de upload não tem prazo

**Locais:** [aquisição da vaga](D:/Nelcota/crates/storage/src/upload.rs:133), [início posterior do timeout](D:/Nelcota/crates/storage/src/upload.rs:142), [32 vagas](D:/Nelcota/crates/storage/src/lib.rs:41), [rotas fora do timeout geral](D:/Nelcota/crates/server/src/lib.rs:105).

O prazo do upload começa apenas em `receive`, depois do `acquire` do semáforo e da reserva de cota. A fila de aquisição não tem limite de espera nem limite de tamanho explícito. A autorização por RLS acontece antes da fila, portanto o problema requer permissão de upload; não é um bypass de políticas.

**Reprodução:** com timeout de **1.000 ms**, 32 uploads autorizados foram mantidos aguardando dados. O 33º aguardou uma vaga e depois iniciou seu próprio prazo: retornou `upload_timeout` após **1.930 ms**. Com mais pedidos em fila, o tempo pode atravessar múltiplos ciclos do prazo configurado. Os arquivos eram temporários no fixture descartável.

**Impacto:** a configuração não garante um prazo total. Sob carga, conexões e requisições pendentes se acumulam, e o painel compartilha as mesmas vagas com a API.

**Correção:** impor prazo e capacidade à admissão. Retornar erro de servidor ocupado quando não houver vaga dentro do orçamento, ou usar um deadline total e repassar apenas o tempo restante ao recebimento. Incluir a reserva de cota no planejamento desse prazo e garantir cancelamento/limpeza dos multipart uploads.

### 7. P2 — Dumps e diretórios de backup dependem da umask do operador

**Locais:** [criação do dump](D:/Nelcota/crates/cli/src/backup.rs:129), [diretórios e cópia de arquivos](D:/Nelcota/crates/cli/src/backup.rs:152), [diretório inicial de backups](D:/Nelcota/crates/cli/src/scaffold.rs:117), [criação do diretório raiz](D:/Nelcota/crates/cli/src/init.rs:145).

`File::create` cria o dump sem modo Unix privado explícito. Os diretórios de backup também são criados sem definir `0700`. Com `umask 022` e diretórios ancestrais acessíveis, um dump novo fica `0644`, legível por outros usuários locais. O rename do arquivo parcial conserva esse modo. Os arquivos do snapshot podem conservar permissões legíveis de suas fontes. A biblioteca padrão documenta criação padrão em `0666`, reduzida pela umask. [OpenOptionsExt::mode](https://doc.rust-lang.org/std/os/unix/fs/trait.OpenOptionsExt.html#tymethod.mode).

**Impacto:** outros usuários locais podem ler todo o banco salvo e arquivos privados, independentemente das políticas RLS. O problema é condicionado às permissões do host: um ancestral `0700`, ACL restritiva ou `umask 077` pode impedir a exposição. Não se trata de leitura remota por uma rota HTTP.

**Correção:** criar dumps já em `0600`, diretórios de backup/snapshot/arquivo em `0700` e aplicar a política às cópias e downloads de backups. Fazer isso na abertura/criação, antes de gravar bytes sensíveis. O helper de `.env` em [envfile.rs](D:/Nelcota/crates/cli/src/envfile.rs:100) também deve abrir o arquivo com o modo privado, em vez de gravar antes do `chmod`.

**Verificação recomendada em Linux:** com `umask 022`, gerar um backup descartável, verificar os modos e tentar leitura como um segundo usuário sem privilégios. Essa reprodução específica de permissões não foi executada no ambiente Windows desta revisão.

## Prioridade baixa

### 8. P3 — Email é normalizado antes de uma validação estrutural suficiente

**Locais:** [normalize_email](D:/Nelcota/crates/auth/src/credentials.rs:12), [signup antes do hash](D:/Nelcota/crates/auth/src/accounts.rs:48), [parsing posterior no mailer](D:/Nelcota/crates/auth/src/mail.rs:60).

A validação exige basicamente um `@`, um ponto no domínio, ausência de whitespace e comprimento máximo. Não rejeita todos os caracteres de controle, domínio iniciado por ponto ou sintaxe de mailbox/display name em um identificador de conta.

**Reprodução:** `normalize_email` aceitou `a\0@b.com`, `<victim@example.com>`, `a@.com` e `a..b@example.com`. Os três últimos foram cadastrados com HTTP 201. O NUL chegou ao banco depois do trabalho de Argon2 e foi rejeitado com HTTP 400 `db_error`, em vez de falhar na validação da credencial.

**Impacto:** dados inconsistentes, contas com endereço inutilizável e diferença de interpretação entre autenticação e envio de email. Não foi demonstrado XSS, injeção de headers ou tomada de conta a partir desses valores; o mailer utiliza a biblioteca lettre para compor headers.

**Correção:** definir e validar um formato de endereço simples coerente em signup, login, painel e OAuth; rejeitar controles e sintaxe de display name antes de hashing/DB. Usar parser de endereço apropriado e preservar a política de normalização existente. Antes de mudar a política, identificar contas antigas que dependam das entradas aceitas. Não apagar caracteres inválidos silenciosamente, pois isso pode alterar a identidade da conta. [Validação de entradas da OWASP](https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html).

### 9. P3 — Cota de storage soma todos os objetos duas vezes por publicação

**Locais:** [admissão](D:/Nelcota/crates/storage/src/quota.rs:68), [soma inicial](D:/Nelcota/crates/storage/src/db.rs:90), [soma na transação](D:/Nelcota/crates/storage/src/quota.rs:134).

Quando `max_total_size` está configurado, a admissão soma todos os tamanhos. A publicação repete a soma após a alteração, enquanto mantém o advisory lock que serializa as publicações. Esse procedimento garante corretamente a cota, inclusive entre processos, mas seu custo cresce com o número total de objetos e acontece a cada upload/substituição.

**Medição:** com 100.001 metadados no banco isolado, `EXPLAIN (ANALYZE, BUFFERS, TIMING OFF)` mostrou `Seq Scan` de **100.001 linhas / 1.810 buffers**, com **5,945 ms** para uma soma, em cache aquecido. Não foi realizado benchmark de throughput com milhões de objetos nem medido o efeito em produção.

**Melhoria:** manter um contador transacional de bytes usados, atualizado por delta na mesma transação da publicação/remoção, com reconciliação periódica. Preservar os testes de concorrência e substituição que protegem a cota. Um índice simples sobre `size` não transforma a soma completa em operação de custo constante.

### 10. P3 — Módulos do painel privado entram no carregamento do login

**Locais:** [imports de funcionalidades](D:/Nelcota/crates/admin/ui/src/App.svelte:8), [preloads emitidos pelo build](D:/Nelcota/crates/admin/ui/dist/index.html:9).

`App.svelte` importa estaticamente o editor de tabelas, usuários, storage geral, migrations, projetos e API. O HTML do build solicita o entrypoint e três chunks em `modulepreload`, mesmo quando a tela mostrada será apenas o login. SQL, Sign-in, StorageBucket e Policies já usam imports dinâmicos, o que é uma boa base para estender esse padrão.

**Medição do build existente:** os quatro arquivos JS pedidos diretamente pelo HTML somam **930.513 bytes**, ou **242.268 bytes com gzip local**. Esse total não inclui CSS, imagens nem outros módulos eventualmente requisitados. O tamanho do chunk `CodeBlock` representa código compartilhado reunido pelo bundler, não apenas o pequeno componente de destaque de texto.

**Melhoria:** carregar páginas privadas por rota após a sessão estar disponível e separar o shell de login das dependências dessas funcionalidades. Medir o novo grafo de carregamento antes/depois. Não foi realizado teste de LCP, CPU de dispositivo móvel ou rede com throttling; os tamanhos acima não são uma medição de velocidade percebida.

## Proteções verificadas e hipóteses descartadas

- Identificadores SQL passam pelo catálogo e são escapados; valores são parâmetros e tipos de casts vêm do catálogo. Não foi identificado caminho de SQL injection nos fluxos revisados.
- RLS permanece a autoridade das operações públicas. O bypass de `service_role` e os poderes SQL do administrador são contratos explícitos, não achados de escalada de privilégio.
- JWT tem seleção explícita de algoritmo/chave/role. `iss`/`aud` informativos e o access token permanecer válido até expirar após logout são comportamentos documentados, não classificados como falhas nesta revisão.
- Cookies administrativos usam HttpOnly/SameSite e Secure no deployment por proxy; o painel faz checagem de origem e possui CSP/nosniff. Não foi encontrado sink de HTML bruto como `@html`, `innerHTML` ou `eval` no código de UI examinado.
- Nomes de objetos são normalizados/validados; bytes usam chaves de bucket/UUID. O prefixo de listagem escapa `%`, `_` e barra invertida antes de `LIKE`.
- As correções anteriores de concorrência no reset/login, refresh do SDK, reserva de cota e worker de Argon2 continuam presentes.
- **Não falta índice para o garbage collector:** `storage.objects.version` possui `UNIQUE`, que cria o índice. A reprodução com 100.001 metadados e 500 versões usou Bitmap Index/Heap Scan, aproximadamente 0,740 ms. Essa hipótese foi descartada.

## Dependências e validação

| Verificação executada | Resultado |
|---|---|
| `cargo audit --json` | 457 dependências examinadas; nenhuma vulnerabilidade não ignorada; base com 1.296 advisories |
| `cargo deny check advisories bans sources` | Aprovado; avisos de versões duplicadas |
| `npm audit --json`, painel | Zero vulnerabilidades reportadas, incluindo dependências de desenvolvimento |
| `npm audit --json`, SDK TypeScript | Zero vulnerabilidades reportadas |
| Reproduções Rust + PostgreSQL 17 | Aprovadas como provas dos comportamentos atuais; fixture removido |

Existe uma exceção explícita em `.cargo/audit.toml` e `deny.toml`: **RUSTSEC-2023-0071**, sobre recuperação de chave privada RSA por timing. A exceção é consistente com os caminhos revisados, que assinam com EdDSA/HS256 e não realizam assinatura/decriptação RSA privada. Ela deve ser reavaliada se esse uso mudar. O advisory continua sem correção publicada; não interpretar a exceção como resolução da vulnerabilidade na dependência. [Advisory RustSec](https://rustsec.org/advisories/RUSTSEC-2023-0071.html).

Não foram repetidas as suítes completas da revisão anterior, nem feito pentest externo, teste de OOM ou benchmark de carga em deployment real. Os achados se apoiam no código atual e nas reproduções descritas. As demais alterações já existentes na árvore de trabalho foram preservadas.

## Sequência de correção sugerida

1. Aplicar limites de resposta/complexidade e conter o cache de consultas (1–3).
2. Compartilhar o orçamento de hashing e redigir as URIs nos logs (4–5).
3. Limitar a admissão de uploads e criar backups com permissões privadas (6–7).
4. Unificar a validação de email e otimizar cota/carregamento inicial (8–10).
