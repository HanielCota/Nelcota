//! Storage on a real S3-compatible service. Ignored by default: it needs
//! one running, e.g.
//!
//! ```sh
//! docker run -d --rm -p 127.0.0.1:9100:9000 \
//!   -e RUSTFS_ACCESS_KEY=nelcotatest -e RUSTFS_SECRET_KEY=nelcotatestsecret rustfs/rustfs
//! # create the bucket `nelcota-files`, then:
//! NELCOTA_TEST_S3_ENDPOINT=http://127.0.0.1:9100 NELCOTA_TEST_S3_BUCKET=nelcota-files \
//! NELCOTA_TEST_S3_ACCESS_KEY_ID=nelcotatest NELCOTA_TEST_S3_SECRET_ACCESS_KEY=nelcotatestsecret \
//!   cargo test -p nelcota-server --test storage_s3 -- --ignored
//! ```

mod common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use common::*;
use nelcota_core::{Config, Secret, config::StorageBackend};

fn s3_config() -> Config {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    Config {
        storage_backend: StorageBackend::S3,
        storage_s3_endpoint: Some(var("NELCOTA_TEST_S3_ENDPOINT")),
        storage_s3_bucket: Some(var("NELCOTA_TEST_S3_BUCKET")),
        storage_s3_region: "us-east-1".into(),
        storage_s3_access_key_id: Some(var("NELCOTA_TEST_S3_ACCESS_KEY_ID")),
        storage_s3_secret_access_key: Some(Secret::new(var("NELCOTA_TEST_S3_SECRET_ACCESS_KEY"))),
        ..Config::default()
    }
}

/// Follows a presigned URL with a plain HTTP/1.1 request.
async fn fetch(url: &str) -> (u16, String, Vec<u8>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rest = url
        .strip_prefix("http://")
        .expect("plain-HTTP test endpoint");
    let (host, path) = rest.split_once('/').unwrap();
    let mut stream = tokio::net::TcpStream::connect(host).await.unwrap();
    stream
        .write_all(
            format!("GET /{path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await
        .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&response[..split]).into_owned();
    let status = head[9..12].parse().unwrap();
    let mut body = response[split + 4..].to_vec();
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        body = dechunk(&body);
    }
    (status, head, body)
}

fn dechunk(mut raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let line = raw.windows(2).position(|w| w == b"\r\n").unwrap();
        let size =
            usize::from_str_radix(std::str::from_utf8(&raw[..line]).unwrap().trim(), 16).unwrap();
        if size == 0 {
            return out;
        }
        out.extend_from_slice(&raw[line + 2..line + 2 + size]);
        raw = &raw[line + 4 + size..];
    }
}

#[tokio::test]
#[ignore = "needs an S3-compatible service (see the module docs)"]
async fn files_on_s3() {
    let store = nelcota_storage::Store::from_config(&s3_config())
        .unwrap()
        .unwrap();
    let mut options = Options::default();
    options.storage.max_file_size = 32 * 1024 * 1024;
    options.store = Some(Arc::new(store));
    let app = TestApp::spawn_with(options).await;
    app.admin_client
        .batch_execute("INSERT INTO storage.buckets (id) VALUES ('docs')")
        .await
        .unwrap();
    let service = service_token();

    // Small (one request) and large (multipart, several 8 MiB parts).
    let large: Vec<u8> = (0..20 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    for (name, body) in [("small.txt", b"hello s3".to_vec()), ("big.bin", large)] {
        let path = format!("/storage/v1/object/docs/{name}");
        let (status, _, reply) = app
            .bytes(
                Method::POST,
                &path,
                Some(&service),
                &[("content-type", "text/plain")],
                body.clone(),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "{}",
            String::from_utf8_lossy(&reply)
        );

        // A download is a redirect to a short presigned URL on the provider.
        let (status, headers, _) = app
            .bytes(Method::GET, &path, Some(&service), &[], vec![])
            .await;
        assert_eq!(status, StatusCode::FOUND);
        let location = headers["location"].to_str().unwrap().to_owned();
        assert!(location.contains("X-Amz-Signature="));
        let (status, head, bytes) = fetch(&location).await;
        assert_eq!(status, 200, "{head}");
        assert_eq!(bytes.len(), body.len());
        assert!(bytes == body, "{name}: content differs");
        assert!(
            head.to_ascii_lowercase().contains("content-disposition: "),
            "{head}"
        );

        let (status, _, _) = app
            .bytes(Method::DELETE, &path, Some(&service), &[], vec![])
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _, _) = fetch(&location).await;
        assert_eq!(status, 404, "the bytes are gone from the bucket");
    }
}
