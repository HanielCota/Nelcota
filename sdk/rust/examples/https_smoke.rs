use nelcota_client::{Client, Error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder("https://example.com").retries(0).build()?;
    match client
        .from("https-smoke")
        .execute::<serde_json::Value>()
        .await
    {
        Ok(_) | Err(Error::Http { .. }) | Err(Error::InvalidResponse { .. }) => {
            println!("Standalone default HTTPS works")
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
