// The migrations in `migrations/` are embedded in the binary at compile time
// (`refinery::embed_migrations!` in src/db.rs), but the compiler does not know
// the crate depends on them: a new file does not trigger a rebuild and the
// binary keeps the old list. This script only tells Cargo to rebuild the crate
// when the folder changes.
fn main() {
    println!("cargo::rerun-if-changed=../../migrations");
}
