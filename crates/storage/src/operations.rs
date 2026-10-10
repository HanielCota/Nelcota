//! File metadata reads, listing and deletion under the caller's policies.
use crate::{
    StorageState, db,
    error::{bucket_not_found, object_not_found},
    path,
    store::Store,
};
use nelcota_core::{ApiError, Claims, db::begin_request};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
const MAX_LIST: i64 = 1000;

#[derive(Deserialize, Serialize)]
pub struct ListedObject {
    pub id: Uuid,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub etag: String,
    pub owner: Option<Uuid>,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Deserialize, Serialize)]
pub struct ObjectListing {
    pub folders: Vec<String>,
    pub objects: Vec<ListedObject>,
}
impl StorageState {
    /// A file under `claims`' policies (the panel passes `service_role`).
    pub async fn object(
        &self,
        claims: &Claims,
        bucket: &str,
        name: &str,
    ) -> Result<db::Object, ApiError> {
        let bucket = path::bucket(bucket)?;
        let name = path::object(name)?;
        let object = {
            let mut client = self.pool.get().await.map_err(ApiError::from_pool)?;
            let tx = begin_request(&mut client, claims)
                .await
                .map_err(|e| ApiError::from_db(e, claims.role()))?;
            let sql = format!(
                "SELECT {} FROM storage.objects WHERE bucket_id = $1 AND name = $2",
                db::OBJECT_COLUMNS
            );
            let row = tx
                .query_opt(&sql, &[&bucket, &name])
                .await
                .map_err(|e| ApiError::from_db(e, claims.role()))?;
            row.as_ref().map(db::Object::from_row)
        }
        .ok_or_else(object_not_found)?;
        Ok(object)
    }
}

impl StorageState {
    /// Deletes a file under `claims`' policies: the row, then the bytes.
    pub async fn delete(&self, claims: &Claims, bucket: &str, name: &str) -> Result<(), ApiError> {
        let bucket = path::bucket(bucket)?;
        let name = path::object(name)?;
        let db_err = |e| ApiError::from_db(e, claims.role());
        let version: Uuid = {
            let mut client = self.pool.get().await.map_err(ApiError::from_pool)?;
            let tx = begin_request(&mut client, claims).await.map_err(db_err)?;
            let row = tx
            .query_opt(
                "DELETE FROM storage.objects WHERE bucket_id = $1 AND name = $2 RETURNING version",
                &[&bucket, &name],
            )
            .await
            .map_err(db_err)?
            .ok_or_else(object_not_found)?;
            tx.commit().await.map_err(db_err)?;
            row.get(0)
        };
        self.store
            .delete_quietly(&Store::key(bucket, version))
            .await;
        Ok(())
    }
}

impl StorageState {
    /// Folders and files right under `prefix`, under `claims`' policies.
    pub async fn list(
        &self,
        claims: &Claims,
        bucket: &str,
        prefix: &str,
        limit: Option<i64>,
        offset: i64,
    ) -> Result<ObjectListing, ApiError> {
        let bucket = path::bucket(bucket)?;
        let prefix = path::prefix(prefix)?;
        let limit = limit.unwrap_or(100).clamp(1, MAX_LIST);
        let offset = offset.max(0);
        let pattern = format!(
            "{}%",
            prefix
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let skip = prefix.chars().count() as i32;
        let db_err = |e| ApiError::from_db(e, claims.role());
        let mut client = self.pool.get().await.map_err(ApiError::from_pool)?;
        let tx = begin_request(&mut client, claims).await.map_err(db_err)?;
        let row = tx
            .query_one(
                include_str!("list.sql"),
                &[&bucket, &pattern, &skip, &limit, &offset],
            )
            .await
            .map_err(db_err)?;
        let text: String = row.get(0);
        serde_json::from_str(&text).map_err(|error| {
            tracing::error!(%error, "invalid object listing");
            ApiError::internal()
        })
    }
}

impl StorageState {
    pub(crate) async fn public_object(
        &self,
        bucket: &str,
        name: &str,
    ) -> Result<db::Object, ApiError> {
        let bucket = path::bucket(bucket)?;
        let name = path::object(name)?;
        match db::bucket(&self.pool, bucket).await? {
            Some(settings) if settings.public => {}
            _ => return Err(bucket_not_found()),
        }
        db::object(&self.pool, bucket, &name)
            .await?
            .ok_or_else(object_not_found)
    }
}
