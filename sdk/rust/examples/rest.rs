use nelcota_client::{Client, rest::Order};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
struct Note {
    id: i64,
    body: String,
}
#[derive(Serialize)]
struct NewNote<'a> {
    body: &'a str,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("NELCOTA_URL").unwrap_or_else(|_| "http://127.0.0.1:8000".into());
    let builder = Client::builder(url);
    let builder = match std::env::var("NELCOTA_ACCESS_TOKEN") {
        Ok(token) => builder.access_token(token),
        Err(_) => builder,
    };
    let client = builder.build()?;
    if let (Ok(email), Ok(password)) = (
        std::env::var("NELCOTA_EMAIL"),
        std::env::var("NELCOTA_PASSWORD"),
    ) {
        client
            .auth()
            .sign_in_with_password(&email, &password)
            .await?;
    }
    let created = client
        .from("notes")
        .insert(&NewNote {
            body: "Hello from Rust",
        })
        .select("id,body")
        .single()
        .execute::<Note>()
        .await?
        .data;
    println!("Created {}: {}", created.id, created.body);
    let rows = client
        .from("notes")
        .select("id,body")
        .order("id", Order::descending())
        .range(0, 9)
        .count_exact()
        .execute::<Vec<Note>>()
        .await?;
    println!("Read {} rows; total {:?}", rows.data.len(), rows.count);
    let changed = client
        .from("notes")
        .update(&json!({"body":"Edited from Rust","extra":null}))
        .eq("id", created.id)
        .select("id,body")
        .single()
        .execute::<Value>()
        .await?;
    println!("Updated note {}", changed.data["id"]);
    client
        .from("notes")
        .delete()
        .eq("id", created.id)
        .execute::<()>()
        .await?;
    Ok(())
}
