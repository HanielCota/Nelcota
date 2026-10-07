// As migrações em `migrations/` são embutidas no binário em tempo de
// compilação (`refinery::embed_migrations!` em src/db.rs), mas o compilador não
// sabe que o crate depende delas: um arquivo novo não dispara a recompilação e
// o binário segue com a lista antiga. Este script só avisa o Cargo para
// reconstruir o crate quando a pasta mudar.
fn main() {
    println!("cargo::rerun-if-changed=../../migrations");
}
