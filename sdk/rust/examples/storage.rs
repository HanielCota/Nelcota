use nelcota_client::{
    Client,
    storage::{ListOptions, UploadOptions},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder(std::env::var("NELCOTA_URL")?)
        .access_token(std::env::var("NELCOTA_ACCESS_TOKEN")?)
        .build()?;
    let user = client.auth().get_user().await?;
    let path = std::env::var("NELCOTA_UPLOAD_PATH")?;
    let file = tokio::fs::File::open(path).await?;
    let name = format!("{}/demo.txt", user.id);
    let files = client.storage().from("files")?;
    let object = files
        // The content type is guessed from the name: demo.txt is text/plain.
        .upload_reader(&name, file, UploadOptions::default())
        .await?;
    println!("Uploaded {} bytes", object.size);
    let downloaded = files.download(&name).await?;
    println!("Downloaded {} bytes", downloaded.len());
    let list = files
        .list(ListOptions {
            prefix: format!("{}/", user.id),
            ..Default::default()
        })
        .await?;
    println!("{} files in your folder", list.objects.len());
    Ok(())
}
