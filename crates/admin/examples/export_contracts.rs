fn main() -> std::io::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/src/lib/generated");
    std::fs::create_dir_all(&root)?;
    let (types, schemas) = nelcota_admin::contracts::exports();
    std::fs::write(root.join("contracts.ts"), types)?;
    std::fs::write(root.join("schemas.json"), schemas)
}
