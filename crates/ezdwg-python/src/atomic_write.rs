//! Replace a complete DWG only after a sibling temporary file has been synced.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Temporary(PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_with(
        path,
        bytes,
        |file, data| file.write_all(data),
        |from, to| fs::rename(from, to),
    )
}

fn atomic_write_with(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    replace: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let permissions = match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() => Some(meta.permissions()),
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "DWG destination must be a regular file, not a directory or symlink",
            ))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };
    // Windows cannot replace a readonly destination, or remove a readonly
    // temporary after failure. Reject before allocating the temporary.
    #[cfg(windows)]
    if permissions.as_ref().is_some_and(|p| p.readonly()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "DWG destination is readonly",
        ));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut candidate = None;
    for _ in 0..128 {
        let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".ezdwg-{}-{id}.tmp", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => {
                candidate = Some((Temporary(temporary), file));
                break;
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    let (temporary, mut file) = candidate.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate DWG temporary file",
        )
    })?;
    let result = (|| {
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        // Apply existing private permissions before any contents are written.
        write(&mut file, bytes)?;
        file.sync_all()
    })();
    // Close before rename or cleanup, including on Windows.
    drop(file);
    result?;
    replace(&temporary.0, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory() -> PathBuf {
        let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ezdwg-atomic-{}-{id}", std::process::id()));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn partial_write_and_replace_failure_preserve_destination_and_clean_temporary() {
        let dir = directory();
        let path = dir.join("drawing.dwg");
        fs::write(&path, b"original").unwrap();
        let result = atomic_write_with(
            &path,
            b"replacement",
            |file, _| {
                file.write_all(b"partial")?;
                Err(io::Error::other("injected disk full"))
            },
            |_, _| panic!("must not replace after a failed write"),
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let result = atomic_write_with(
            &path,
            b"replacement",
            |file, data| file.write_all(data),
            |_, _| {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected replace failure",
                ))
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn creates_and_replaces_complete_file() {
        let dir = directory();
        let path = dir.join("drawing.dwg");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn private_permissions_apply_before_writing_and_survive_replacement() {
        use std::os::unix::fs::PermissionsExt;
        let dir = directory();
        let path = dir.join("drawing.dwg");
        fs::write(&path, b"original").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        atomic_write_with(
            &path,
            b"private",
            |file, data| {
                assert_eq!(file.metadata()?.permissions().mode() & 0o777, 0o600);
                file.write_all(data)
            },
            |from, to| fs::rename(from, to),
        )
        .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
