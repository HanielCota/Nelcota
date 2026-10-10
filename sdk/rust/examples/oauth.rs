use nelcota_client::{Client, auth::OAuthProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder(std::env::var("NELCOTA_URL")?).build()?;
    let flow = client.auth().begin_oauth(
        OAuthProvider::Github,
        &std::env::var("NELCOTA_OAUTH_REDIRECT_URL")?,
    )?;
    println!("Open this URL in a browser: {}", flow.url);
    println!("Paste the full callback URL here:");
    let callback = tokio::task::spawn_blocking(|| {
        let mut value = String::new();
        std::io::stdin().read_line(&mut value).map(|_| value)
    })
    .await??;
    let mut callback = url::Url::parse(callback.trim())?;
    if let Some(session) = client.auth().handle_redirect(&mut callback, flow).await? {
        println!("Signed in as {}", session.user.email);
    }
    Ok(())
}
