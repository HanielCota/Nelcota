//! Execução do DDL gerado pelo painel, ou só a prévia dele.

use axum::Json;
use serde_json::{Value, json};

use crate::{AdminState, ApiError, api::user_query_error};

/// Com `preview`, devolve o SQL sem executar. Sem, executa tudo numa
/// transação e recarrega o catálogo antes de responder: quem chamou já
/// enxerga a tabela/coluna nova na API (sem esperar o NOTIFY).
///
/// Cada comando vai pelo protocolo estendido (`execute`), que recusa mais
/// de um comando por vez: uma expressão como `true); DROP TABLE x; --`
/// falha em vez de executar o que vem depois.
pub async fn apply(
    state: &AdminState,
    statements: Vec<String>,
    preview: bool,
    message: &str,
) -> Result<Json<Value>, ApiError> {
    if preview {
        return Ok(Json(json!({ "sql": statements })));
    }
    {
        let mut client = state.db.get().await?;
        let tx = client.transaction().await?;
        for statement in &statements {
            tx.execute(statement.as_str(), &[])
                .await
                .map_err(user_query_error)?;
        }
        // Registro para "Gerar migração" (ver migrations.rs), na mesma
        // transação: DDL que falha não é registrado, e vice-versa.
        tx.execute(
            "INSERT INTO nelcota.panel_changes (summary, statements) VALUES ($1, $2)",
            &[&message, &statements],
        )
        .await?;
        tx.commit().await.map_err(user_query_error)?;
        // A conexão volta ao pool aqui; a recarga usa outra.
    }
    // O texto do DDL não vai para o log (pode conter dados), só o volume.
    tracing::info!(comandos = statements.len(), "DDL aplicado pelo painel");
    state
        .catalog
        .reload(&state.db)
        .await
        .map_err(|e| ApiError::from(format!("DDL aplicado, mas o catálogo não recarregou: {e}")))?;
    Ok(Json(json!({ "message": message, "sql": statements })))
}
