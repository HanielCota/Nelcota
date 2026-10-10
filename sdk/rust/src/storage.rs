//! File transfers and buckets. Uploads send raw bytes, not multipart forms.
use crate::{Client, Error, Result, encoding, http::Spec};
use bytes::Bytes;
use reqwest::{
    Method,
    header::{CONTENT_TYPE, HeaderValue, IF_NONE_MATCH, RANGE},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bucket {
    pub id: String,
    pub public: bool,
    pub file_size_limit: Option<u64>,
    pub allowed_mime_types: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
}

/// PUT replaces the complete settings; omitted limits are cleared.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BucketSettings {
    pub public: bool,
    pub file_size_limit: Option<u64>,
    pub allowed_mime_types: Option<Vec<String>>,
}
impl BucketSettings {
    fn validate(&self) -> Result<()> {
        if self.file_size_limit == Some(0) {
            return Err(Error::Usage("file_size_limit must be positive".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct StoredObject {
    pub id: String,
    pub bucket: String,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub etag: String,
    pub public_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ListedObject {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub etag: String,
    pub owner: Option<String>,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Listing {
    pub folders: Vec<String>,
    pub objects: Vec<ListedObject>,
}

#[derive(Clone, Debug)]
pub struct UploadOptions {
    pub content_type: String,
    pub upsert: bool,
}
impl Default for UploadOptions {
    fn default() -> Self {
        Self {
            content_type: "application/octet-stream".into(),
            upsert: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    /// Inclusive byte range; None for the end means to EOF.
    pub range: Option<(u64, Option<u64>)>,
    pub if_none_match: Option<String>,
    pub download: bool,
}

#[derive(Clone, Debug)]
pub struct ListOptions {
    pub prefix: String,
    pub limit: u16,
    pub offset: u64,
}
impl Default for ListOptions {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            limit: 100,
            offset: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StorageClient {
    pub(crate) client: Client,
}
impl StorageClient {
    pub fn from(&self, bucket: &str) -> Result<BucketFiles> {
        encoding::bucket(bucket)?;
        Ok(BucketFiles {
            client: self.client.clone(),
            bucket: bucket.into(),
        })
    }
    pub async fn list_buckets(&self) -> Result<Vec<Bucket>> {
        self.client
            .json::<Vec<Bucket>>(Spec::new(Method::GET, "/storage/v1/bucket"))
            .await
            .map(|r| r.data)
    }
    pub async fn get_bucket(&self, id: &str) -> Result<Bucket> {
        encoding::bucket(id)?;
        self.client
            .json::<Bucket>(Spec::new(Method::GET, format!("/storage/v1/bucket/{id}")))
            .await
            .map(|r| r.data)
    }
    pub async fn create_bucket(&self, id: &str, settings: BucketSettings) -> Result<Bucket> {
        encoding::bucket(id)?;
        settings.validate()?;
        let mut value = serde_json::to_value(settings).map_err(|e| Error::Usage(e.to_string()))?;
        value["id"] = Value::String(id.into());
        self.client
            .json::<Bucket>(Spec::new(Method::POST, "/storage/v1/bucket").json(value))
            .await
            .map(|r| r.data)
    }
    pub async fn update_bucket(&self, id: &str, settings: BucketSettings) -> Result<Bucket> {
        encoding::bucket(id)?;
        settings.validate()?;
        self.client
            .json::<Bucket>(
                Spec::new(Method::PUT, format!("/storage/v1/bucket/{id}"))
                    .json(serde_json::to_value(settings).map_err(|e| Error::Usage(e.to_string()))?),
            )
            .await
            .map(|r| r.data)
    }
    pub async fn delete_bucket(&self, id: &str) -> Result<()> {
        encoding::bucket(id)?;
        self.client
            .bytes(Spec::new(
                Method::DELETE,
                format!("/storage/v1/bucket/{id}"),
            ))
            .await
            .map(|_| ())
    }
}

#[derive(Clone, Debug)]
pub struct BucketFiles {
    client: Client,
    bucket: String,
}
impl BucketFiles {
    fn path(&self, kind: &str, name: &str) -> Result<String> {
        Ok(format!(
            "/storage/v1/object/{kind}{}/{}",
            self.bucket,
            encoding::object(name)?
        ))
    }
    pub async fn upload(
        &self,
        name: &str,
        body: impl Into<reqwest::Body>,
        options: UploadOptions,
    ) -> Result<StoredObject> {
        let mut spec = Spec::new(
            if options.upsert {
                Method::PUT
            } else {
                Method::POST
            },
            self.path("", name)?,
        );
        let content_type = HeaderValue::from_str(&options.content_type)
            .map_err(|_| Error::Usage("invalid content type header".into()))?;
        if !options
            .content_type
            .split(';')
            .next()
            .unwrap_or_default()
            .contains('/')
        {
            return Err(Error::Usage("content type requires type/subtype".into()));
        }
        spec.headers.insert(CONTENT_TYPE, content_type);
        spec.body = Some(body.into());
        spec.timeout = Some(Duration::ZERO);
        self.client.json::<StoredObject>(spec).await.map(|r| r.data)
    }
    /// Stream a Tokio reader (for example a File), without buffering the upload.
    pub async fn upload_reader<R>(
        &self,
        name: &str,
        reader: R,
        options: UploadOptions,
    ) -> Result<StoredObject>
    where
        R: tokio::io::AsyncRead + Send + Unpin + 'static,
    {
        self.upload(
            name,
            reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(reader)),
            options,
        )
        .await
    }
    fn open_spec(&self, name: &str, options: OpenOptions) -> Result<Spec> {
        let mut spec = Spec::new(Method::GET, self.path("", name)?);
        spec.timeout = Some(Duration::ZERO);
        if let Some((start, end)) = options.range {
            if end.is_some_and(|end| end < start) {
                return Err(Error::Usage("range end must be >= start".into()));
            }
            let range = format!(
                "bytes={start}-{}",
                end.map(|v| v.to_string()).unwrap_or_default()
            );
            spec.headers.insert(
                RANGE,
                HeaderValue::from_str(&range).map_err(|_| Error::Usage("invalid range".into()))?,
            );
        }
        if let Some(etag) = options.if_none_match {
            spec.headers.insert(
                IF_NONE_MATCH,
                HeaderValue::from_str(&etag).map_err(|_| Error::Usage("invalid ETag".into()))?,
            );
        }
        if options.download {
            spec.params.push(("download".into(), String::new()));
        }
        Ok(spec)
    }
    /// Raw streaming response: handle 206/304 and body failures after return yourself.
    /// Cross-origin S3 redirects are followed without forwarding the bearer token.
    pub async fn open(&self, name: &str, options: OpenOptions) -> Result<reqwest::Response> {
        self.client.raw(self.open_spec(name, options)?).await
    }
    /// Buffer the complete file. Use open for large files or conditional downloads.
    pub async fn download(&self, name: &str) -> Result<Bytes> {
        self.client
            .bytes(self.open_spec(name, OpenOptions::default())?)
            .await
            .map(|r| r.data)
    }
    pub async fn remove(&self, name: &str) -> Result<()> {
        self.client
            .bytes(Spec::new(Method::DELETE, self.path("", name)?))
            .await
            .map(|_| ())
    }
    pub async fn list(&self, options: ListOptions) -> Result<Listing> {
        if !(1..=1000).contains(&options.limit) {
            return Err(Error::Usage("listing limit must be 1–1000".into()));
        }
        let body = json!({"prefix": encoding::prefix(&options.prefix)?, "limit":options.limit, "offset":options.offset});
        self.client
            .json::<Listing>(
                Spec::new(
                    Method::POST,
                    format!("/storage/v1/object/list/{}", self.bucket),
                )
                .json(body),
            )
            .await
            .map(|r| r.data)
    }
    pub async fn create_signed_url(&self, name: &str, expires_in: u64) -> Result<Url> {
        if !(1..=604_800).contains(&expires_in) {
            return Err(Error::Usage(
                "signed URL duration must be 1–604800 seconds".into(),
            ));
        }
        let response = self
            .client
            .json::<Value>(
                Spec::new(Method::POST, self.path("sign/", name)?)
                    .json(json!({"expires_in":expires_in})),
            )
            .await?;
        let relative = response.data["signed_url"]
            .as_str()
            .filter(|url| url.starts_with("/storage/v1/object/sign/"))
            .ok_or_else(|| Error::InvalidResponse {
                status: response.status,
                message: "unexpected signed URL".into(),
            })?;
        self.client.url(relative)
    }
    /// Construct a public URL locally. The bucket must allow public reads.
    pub fn public_url(&self, name: &str) -> Result<Url> {
        self.client.url(&self.path("public/", name)?)
    }
}
