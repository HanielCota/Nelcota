//! The disk store's root is private, including existing installations. Keeping
//! it unsearchable by other users protects both published and staged uploads,
//! regardless of the file modes chosen by object_store or the process umask.
use std::{fs, io, path::Path};

pub(crate) fn create(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
