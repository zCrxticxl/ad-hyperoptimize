//! Stable lock files serialize transactions across app and broker processes.
use std::fs::{File, OpenOptions};
use std::path::Path;

pub(crate) fn lock_at(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| format!("opening transaction lock {}: {e}", path.display()))?;
    file.lock()
        .map_err(|e| format!("locking {}: {e}", path.display()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn child_lock_probe() {
        let Some(path) = std::env::var_os("AD_TEST_TRANSACTION_LOCK") else {
            return;
        };
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        assert!(matches!(
            file.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
    }

    #[test]
    fn excludes_other_process_and_releases_on_drop() {
        let path = std::env::temp_dir().join(format!(
            "ad-lock-{}-{}.lock",
            std::process::id(),
            rand::random::<u64>()
        ));
        let guard = lock_at(&path).unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "transaction::tests::child_lock_probe",
                "--nocapture",
            ])
            .env("AD_TEST_TRANSACTION_LOCK", &path)
            .status()
            .unwrap();
        assert!(status.success());
        drop(guard);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.try_lock().unwrap();
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
}
