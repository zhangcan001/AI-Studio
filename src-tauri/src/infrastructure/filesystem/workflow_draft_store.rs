use crate::application::ports::workflow_draft_store::WorkflowDraftStore;
use std::{fs, io::Write, path::PathBuf};

pub struct FileSystemWorkflowDraftStore {
    root: PathBuf,
}

impl FileSystemWorkflowDraftStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err("invalid draft identity".to_owned());
        }
        Ok(self.root.join(format!("{id}.json")))
    }
}

impl WorkflowDraftStore for FileSystemWorkflowDraftStore {
    fn save_raw(&self, id: &str, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("draft source exceeds size limit".to_owned());
        }
        let path = self.path(id)?;
        fs::create_dir_all(&self.root).map_err(|_| "draft directory unavailable")?;
        // Immutable source: never replace an existing draft with unrelated bytes.
        if path.exists() {
            return if self.read_raw(id)? == bytes {
                Ok(())
            } else {
                Err("draft source identity conflict".to_owned())
            };
        }
        let temporary = self.root.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|_| "draft source write failed")?;
            file.write_all(bytes)
                .map_err(|_| "draft source write failed")?;
            file.sync_all().map_err(|_| "draft source sync failed")?;
            // Hard-link publishes the complete file atomically without replacing an existing source.
            fs::hard_link(&temporary, &path).map_err(|_| "draft source publication failed")?;
            Ok(())
        })();
        let _ = fs::remove_file(temporary);
        result
    }

    fn read_raw(&self, id: &str) -> Result<Vec<u8>, String> {
        let path = self.path(id)?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| "draft source unavailable")?;
        if !metadata.file_type().is_file() || metadata.len() > 32 * 1024 * 1024 {
            return Err("invalid draft source file".to_owned());
        }
        fs::read(path).map_err(|_| "draft source read failed".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pr_e_draft_raw_survives_store_reload_and_rejects_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileSystemWorkflowDraftStore::new(dir.path().to_owned());
        let raw = br#"{"nodes":[],"source":"original"}"#;
        store.save_raw("onb_test", raw).unwrap();
        store.save_raw("onb_test", raw).unwrap();
        assert!(store.save_raw("onb_test", b"replacement").is_err());
        let reloaded = FileSystemWorkflowDraftStore::new(dir.path().to_owned());
        assert_eq!(reloaded.read_raw("onb_test").unwrap(), raw);
        assert!(store.read_raw("../outside").is_err());
    }
}
