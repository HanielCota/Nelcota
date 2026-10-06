# API REST

Gerada do schema exposto (`NELCOTA_DB_SCHEMA`, padrão `public`). Toda tabela,
view e função do schema vira endpoint, e **quem decide o acesso é o Postgres**:
GRANTs dizem quais roles podem usar cada objeto e o RLS diz quais linhas cada
usuário enxerga.

> Tabelas novas não têm GRANT para `anon`/`authenticated`: até você conceder,
> a API responde 401/403. Isso é intencional.
>
> ```sql
> GRANT SELECT, INSERT, UPDATE, DELETE ON public.minha_tabela TO authenticated;
> ALTER TABLE public.minha_tabela ENABLE ROW LEVEL SECURITY;
> CREATE POLICY ... ;
> ```

## Leitura: `GET /rest/v1/{tabela}`

| Parâmetro | Exemplo | Efeito |
|---|---|---|
| `select` | `select=id,titulo` | colunas (padrão `*`) |
| `{coluna}` | `preco=gt.10` | filtro (vários são combinados com AND) |
| `order` | `order=criado_em.desc.nullslast,id` | ordenação |
| `limit`, `offset` | `limit=20&offset=40` | paginação |

### Operadores

| Operador | SQL | Exemplo |
|---|---|---|
| `eq`, `neq` | `=`, `<>` | `status=eq.ativo` |
| `gt`, `gte`, `lt`, `lte` | `>`, `>=`, `<`, `<=` | `idade=gte.18` |
| `like`, `ilike` | `LIKE`, `ILIKE` (`*` vira `%`) | `nome=ilike.*silva*` |
| `in` | `= ANY(...)` | `id=in.(1,2,3)`, `nome=in.("a,b",c)` |
| `is` | `IS NULL/TRUE/FALSE/UNKNOWN` | `apagado_em=is.null` |
| `not.` | `NOT (...)` | `status=not.eq.cancelado`, `x=not.is.null` |

Os valores chegam ao Postgres como parâmetros e são convertidos para o tipo da
coluna pelo próprio Postgres (`$1::text::<tipo>`). Valor inválido para o tipo
dá 400.

### Respostas

- Corpo: array JSON montado pelo Postgres (`json_agg`).
- `Content-Range: 0-19/*`, ou `0-19/137` com `Prefer: count=exact`.
- `NELCOTA_MAX_ROWS` (opcional) limita o número de linhas por leitura.

## Escrita

| Verbo | Corpo | Filtros | Sem `return=representation` | Com `return=representation` |
|---|---|---|---|---|
| `POST` | objeto ou array | não aceita | 201 vazio | 201 + array |
| `PATCH` | objeto | **obrigatórios** | 204 | 200 + array |
| `DELETE` | - | **obrigatórios** | 204 | 200 + array |

- `select=` também escolhe as colunas da representação.
- No `POST`, colunas ausentes recebem o `DEFAULT`, mesmo num lote com chaves
  diferentes entre os objetos.
- `PATCH`/`DELETE` sem filtro são recusados (400), para evitar alterar ou
  apagar a tabela inteira por engano. Para isso de propósito, use um filtro
  explícito (`?id=not.is.null`).
- `return=representation` executa `RETURNING`, que exige permissão de
  `SELECT` (GRANT + policy) nas linhas escritas.

## Funções: `POST /rest/v1/rpc/{funcao}`

Corpo: objeto com os argumentos **nomeados** (`{"a": 1, "b": 2}`); argumentos
com `DEFAULT` são opcionais. Com sobrecarga, vale a assinatura cujos nomes
batem com as chaves.

| Retorno da função | Resposta |
|---|---|
| `SETOF`/`TABLE` | array JSON |
| escalar ou composto | valor JSON |
| `void` | 204 |

`RAISE EXCEPTION` vira 400 com a mensagem. A função roda com a role do JWT
(a menos que seja `SECURITY DEFINER`): `auth.uid()` funciona dentro dela.

> ⚠️ Por padrão, o Postgres concede `EXECUTE` de funções novas a `PUBLIC`
> (inclusive `anon`). Para funções sensíveis:
> `REVOKE EXECUTE ON FUNCTION f() FROM PUBLIC; GRANT EXECUTE ON FUNCTION f() TO authenticated;`

## OpenAPI: `GET /rest/v1/`

Documento OpenAPI 3.0 gerado do catálogo e **filtrado pela role do request**:
`anon` só vê o que `anon` pode usar.

## Recarga do catálogo

A API guarda a introspecção em memória. Um event trigger (migração V3) avisa o
servidor a cada DDL e a recarga acontece em ~100 ms. Sem superusuário (alguns
Postgres gerenciados), recarregue à mão:

```sql
NOTIFY nelcota, 'reload schema';
```

## Erros

`{"code": "...", "message": "..."}`

| Status | Quando |
|---|---|
| 400 `invalid_query` | coluna/operador/ordem inválidos na URL |
| 400 `invalid_body` | corpo não é JSON válido |
| 400 `db_error` | valor inválido para o tipo, check, not null, coluna gerada, `RAISE EXCEPTION` |
| 401 | JWT inválido, ou `anon` sem permissão |
| 403 | role sem permissão, ou policy violada na escrita |
| 404 `not_found` | tabela/função fora do schema exposto |
| 409 | chave única ou FK violada |
| 504 | `statement_timeout` (`NELCOTA_STATEMENT_TIMEOUT_SECS`, padrão 10 s) |

## Fora do MVP

Embed de relações (`select=*,pedidos(*)`), `or=`/`and=`, upsert
(`on_conflict`), `GET` em `/rpc`, `Accept: application/vnd.pgrst.object+json`.
