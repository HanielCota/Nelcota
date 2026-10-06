//! API REST. No Marco 1 existe só uma rota fixa sobre a tabela de exemplo,
//! para provar o fluxo JWT → role → RLS de ponta a ponta. A introspecção do
//! catálogo e o CRUD genérico entram no Marco 3 e substituem esta rota.

use axum::{
    Router,
    extract::{FromRef, State},
    http::header,
    response::IntoResponse,
    routing::get,
};
use deadpool_postgres::Pool;
use nelcota_auth::{Auth, SharedVerifier};
use nelcota_core::{ApiError, db};

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    Pool: FromRef<S>,
    SharedVerifier: FromRef<S>,
{
    Router::new().route("/rest/v1/todos", get(list_todos))
}

/// `GET /rest/v1/todos`: o Postgres monta o JSON; a API só repassa os bytes.
async fn list_todos(
    State(pool): State<Pool>,
    Auth(claims): Auth,
) -> Result<impl IntoResponse, ApiError> {
    let role = claims.role();
    let db_err = |e| ApiError::from_db(e, role);

    let mut client = pool.get().await.map_err(ApiError::from_pool)?;
    let tx = db::begin_request(&mut client, &claims)
        .await
        .map_err(db_err)?;
    let statement = tx
        .prepare_cached(
            "SELECT coalesce(json_agg(t ORDER BY t.id), '[]')::text FROM public.todos t",
        )
        .await
        .map_err(db_err)?;
    let body: String = tx.query_one(&statement, &[]).await.map_err(db_err)?.get(0);
    tx.commit().await.map_err(db_err)?;

    Ok(([(header::CONTENT_TYPE, "application/json")], body))
}
