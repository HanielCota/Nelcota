//! Files containing credentials or backups are private from the first write.
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

pub(crate) fn file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Also restrict an existing file, before truncating or writing it.
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.set_len(0)?;
    Ok(file)
}

pub(crate) fn dir(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    restrict(path, true)
}

pub(crate) fn write(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    use io::Write;
    replace(path, |file| file.write_all(contents.as_ref()))
}

/// Keep the previous configuration until its replacement is fully written.
/// A same-directory temporary file makes the final rename atomic, including
/// on Windows. Dropping it on any earlier error removes the partial contents.
fn replace(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(".nelcota-")
        .tempfile_in(parent)?;
    restrict(temporary.path(), false)?;
    write(temporary.as_file_mut())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

pub(crate) fn copy(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> io::Result<u64> {
    let mut source = File::open(source)?;
    let mut copied = 0;
    replace(destination.as_ref(), |destination| {
        copied = io::copy(&mut source, destination)?;
        Ok(())
    })?;
    Ok(copied)
}

fn restrict(path: &Path, directory: bool) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(match directory {
                true => 0o700,
                false => 0o600,
            }),
        )?;
    }
    #[cfg(not(unix))]
    let _ = (path, directory);
    Ok(())
}

/// Downloads run with umask 077; also seal pre-existing members of the bundle.
pub(crate) fn seal_tree(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "symlink in private backup bundle",
        ));
    }
    restrict(path, meta.is_dir())?;
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            seal_tree(&entry?.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_replacement_preserves_credentials_and_removes_partial_file() {
        use io::Write;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".env");
        write(&path, b"ORIGINAL=secret\n").unwrap();
        let result = replace(&path, |file| {
            file.write_all(b"PARTIAL=")?;
            Err(io::Error::other("simulated full disk"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"ORIGINAL=secret\n");
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
    #[test]
    fn directories_are_private_when_created_and_reopened() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("nested/storage");
        dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }
    #[test]
    fn copies_and_overwrites_private_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("backup");
        dir(&root).unwrap();
        let source = temp.path().join("source");
        fs::write(&source, b"secret").unwrap();
        let destination = root.join("copy");
        copy(&source, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o644)).unwrap();
        }
        write(&destination, b"new").unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
