# REST API

Generated from the exposed schema (`NELCOTA_DB_SCHEMA`, default `public`). Every
table, view and function in the schema becomes an endpoint, and **Postgres
decides access**: GRANTs say which roles may use each object and RLS says which
rows each user sees.

> New tables have no GRANT for `anon`/`authenticated`: until you grant it, the
> API answers 401/403. This is intentional.
>
> ```sql
> GRANT SELECT, INSERT, UPDATE, DELETE ON public.my_table TO authenticated;
> ALTER TABLE public.my_table ENABLE ROW LEVEL SECURITY;
> CREATE POLICY ... ;
> ```

## Reads: `GET /rest/v1/{table}`

| Parameter | Example | Effect |
|---|---|---|
| `select` | `select=id,title` | columns (default `*`) |
| `{column}` | `price=gt.10` | filter (several are combined with AND) |
| `or`, `and` | `or=(status.eq.paid,status.eq.shipped)` | group of filters, see below |
| `order` | `order=created_at.desc.nullslast,id` | ordering |
| `limit`, `offset` | `limit=20&offset=40` | pagination |

### Operators

| Operator | SQL | Example |
|---|---|---|
| `eq`, `neq` | `=`, `<>` | `status=eq.active` |
| `gt`, `gte`, `lt`, `lte` | `>`, `>=`, `<`, `<=` | `age=gte.18` |
| `like`, `ilike` | `LIKE`, `ILIKE` (`*` becomes `%`) | `name=ilike.*silva*` |
| `in` | `= ANY(...)` | `id=in.(1,2,3)`, `name=in.("a,b",c)` |
| `is` | `IS NULL/TRUE/FALSE/UNKNOWN` | `deleted_at=is.null` |
| `not.` | `NOT (...)` | `status=not.eq.cancelled`, `x=not.is.null` |

Values reach Postgres as parameters and Postgres itself converts them to the
column type (`$1::text::<type>`). A value invalid for the type gives 400.

### Combining filters: `or` and `and`

Same syntax as PostgREST. Inside the parentheses each filter is written
`column.operator.value`:

```
?or=(status.eq.paid,status.eq.shipped)
?or=(stock.eq.0,and(price.gt.100,featured.is.true))
?not.or=(status.eq.cancelled,deleted_at.not.is.null)
?or=(price.gt.100,price.lt.3)&stock=gt.0      # the group is ANDed with the rest
```

- `and(...)` and `or(...)` nest; `not.` before a group negates it, and
  `column.not.operator.value` negates one filter.
- A value with commas or parentheses goes in double quotes, with `\"` and `\\`
  as escapes: `or=(name.eq."Ruler, 30cm",name.eq."f(x)")`. `in` lists keep
  their own quoting.
- At most 8 nested levels and 100 filters per group; beyond that, 400.
- A column literally named `or` or `and` cannot be used as a direct filter,
  only inside a group (`and=(or.eq.1)`), as in PostgREST.
- RLS still applies: a group can only narrow what the policies allow, never
  widen it, on reads and on `PATCH`/`DELETE`.

### Responses

- Body: a JSON array built by Postgres (`json_agg`).
- `Content-Range: 0-19/*`, or `0-19/137` with `Prefer: count=exact`.
- `NELCOTA_MAX_ROWS` (optional) caps the number of rows per read.

## Writes

| Verb | Body | Filters | Without `return=representation` | With `return=representation` |
|---|---|---|---|---|
| `POST` | object or array | not accepted | 201 empty | 201 + array |
| `PATCH` | object | **required** | 204 | 200 + array |
| `DELETE` | - | **required** | 204 | 200 + array |

- `select=` also picks the columns of the representation.
- On `POST`, missing columns get their `DEFAULT`, even in a batch whose
  objects have different keys.
- `PATCH`/`DELETE` without a filter are refused (400), to avoid changing or
  deleting the whole table by mistake. To do that on purpose, use an explicit
  filter (`?id=not.is.null`).
- `return=representation` runs `RETURNING`, which requires `SELECT`
  permission (GRANT + policy) on the written rows.

### Upsert: create or update in one call

```
POST /rest/v1/products
Prefer: resolution=merge-duplicates
[{ "id": 1, "price": 3.00 }, { "id": 99, "name": "Eraser", "price": 1.00 }]
```

- `Prefer: resolution=merge-duplicates` updates a row whose key already exists,
  with the columns the object sent (the others keep their values; the key
  itself is never changed). `resolution=ignore-duplicates` keeps the existing
  row; with `return=representation`, only the rows actually written come back.
- The key is the primary key, or `?on_conflict=col1,col2`, which must match a
  unique constraint (otherwise 400). `on_conflict` without `resolution` is a
  400. The response confirms with `Preference-Applied`.
- A batch that repeats a key gets a 400 from Postgres (a row cannot be updated
  twice in one statement).
- A `GENERATED ALWAYS AS IDENTITY` column cannot receive values, so it cannot
  be the upsert key: use `BY DEFAULT`, or another unique column with
  `on_conflict`.
- RLS still decides: on a conflict, the existing row must pass the `UPDATE`
  policy. A user cannot take over someone else's row through an upsert (403).

## Functions: `POST /rest/v1/rpc/{function}`

Body: an object with the **named** arguments (`{"a": 1, "b": 2}`); arguments
with a `DEFAULT` are optional. With overloads, the signature whose names match
the keys wins.

| Function returns | Response |
|---|---|
| `SETOF`/`TABLE` | JSON array |
| scalar or composite | JSON value |
| `void` | 204 |

`RAISE EXCEPTION` becomes 400 with the message. The function runs with the
JWT's role (unless it is `SECURITY DEFINER`): `auth.uid()` works inside it.

> **Warning:** By default, Postgres grants `EXECUTE` on new functions to
> `PUBLIC` (including `anon`). For sensitive functions:
> `REVOKE EXECUTE ON FUNCTION f() FROM PUBLIC; GRANT EXECUTE ON FUNCTION f() TO authenticated;`

## OpenAPI: `GET /rest/v1/`

An OpenAPI 3.0 document generated from the catalog and **filtered by the
request's role**: `anon` only sees what `anon` may use.

## Catalog reload

The API keeps the introspection in memory. An event trigger (migration V3)
notifies the server on every DDL and the reload happens in ~100 ms. Without a
superuser (some managed Postgres services), reload by hand:

```sql
NOTIFY nelcota, 'reload schema';
```

## Errors

`{"code": "...", "message": "..."}`

| Status | When |
|---|---|
| 400 `invalid_query` | invalid column/operator/order in the URL |
| 400 `invalid_body` | the body is not valid JSON |
| 400 `db_error` | value invalid for the type, check, not null, generated column, `RAISE EXCEPTION` |
| 401 | invalid JWT, or `anon` without permission |
| 403 | role without permission, or a policy violated on write |
| 404 `not_found` | table/function outside the exposed schema |
| 409 | unique key or FK violated |
| 504 | `statement_timeout` (`NELCOTA_STATEMENT_TIMEOUT_SECS`, default 10 s) |

## Out of the MVP

Relation embedding (`select=*,orders(*)`), `GET` on `/rpc`,
`Accept: application/vnd.pgrst.object+json`.
