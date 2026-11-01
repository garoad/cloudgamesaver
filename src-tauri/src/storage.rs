use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Debug)]
pub struct LocalFile {
    root: String,
    relative_path: String,
}

pub fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains(['\\', '\0'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || Path::new(path).is_absolute()
        || Path::new(path)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(format!("안전하지 않은 세이브 파일 경로: {}", path));
    }
    Ok(())
}

impl LocalFile {
    pub fn new(root: &str, relative_path: &str) -> Result<Self, String> {
        validate_relative_path(relative_path)?;
        Ok(Self {
            root: root.to_owned(),
            relative_path: relative_path.to_owned(),
        })
    }

    fn filesystem_path(&self) -> Result<PathBuf, String> {
        if self.root.starts_with("content://") {
            return Err("Android 폴더는 Android 앱에서 연결해주세요.".to_owned());
        }
        Ok(Path::new(&self.root).join(&self.relative_path))
    }

    pub async fn modified(&self, app: &AppHandle) -> Result<Option<DateTime<Utc>>, String> {
        #[cfg(target_os = "android")]
        if self.root.starts_with("content://") {
            let stat = super::android_storage::stat(app, &self.root, &self.relative_path)?;
            return if stat.exists {
                if stat.modified_millis <= 0 {
                    return Err("수정 시간을 읽을 수 없습니다. 기기의 로컬 저장소에서 세이브 폴더를 선택해주세요.".to_owned());
                }
                DateTime::from_timestamp_millis(stat.modified_millis)
                    .map(Some)
                    .ok_or_else(|| "세이브 파일 수정 시간을 읽지 못했습니다.".to_owned())
            } else {
                Ok(None)
            };
        }
        let _ = app;
        match tokio::fs::metadata(self.filesystem_path()?).await {
            Ok(metadata) => Ok(Some(metadata.modified().map_err(|e| e.to_string())?.into())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn open_read(&self, app: &AppHandle) -> Result<tokio::fs::File, String> {
        #[cfg(target_os = "android")]
        if self.root.starts_with("content://") {
            return super::android_storage::open(app, &self.root, &self.relative_path, "r")
                .map(tokio::fs::File::from_std);
        }
        let _ = app;
        tokio::fs::File::open(self.filesystem_path()?)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn read(&self, app: &AppHandle) -> Result<Vec<u8>, String> {
        use tokio::io::AsyncReadExt;
        let mut file = self.open_read(app).await?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .await
            .map_err(|e| e.to_string())?;
        Ok(bytes)
    }

    pub async fn write(&self, app: &AppHandle, bytes: &[u8]) -> Result<(), String> {
        #[cfg(target_os = "android")]
        if self.root.starts_with("content://") {
            let mut file = tokio::fs::File::from_std(super::android_storage::open(
                app,
                &self.root,
                &self.relative_path,
                "rwt",
            )?);
            file.write_all(bytes).await.map_err(|e| e.to_string())?;
            return file.flush().await.map_err(|e| e.to_string());
        }
        let _ = app;
        let path = self.filesystem_path()?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        let mut file = tokio::fs::File::create(path)
            .await
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).await.map_err(|e| e.to_string())?;
        file.flush().await.map_err(|e| e.to_string())
    }
}

pub async fn ensure_folder(app: &AppHandle, root: &str) -> Result<(), String> {
    #[cfg(target_os = "android")]
    if root.starts_with("content://") {
        return super::android_storage::validate_tree(app, root);
    }
    let _ = app;
    if root.starts_with("content://") {
        return Err("Android 폴더는 Android 앱에서 연결해주세요.".to_owned());
    }
    if root.is_empty() {
        return Err("세이브 폴더를 선택해주세요.".to_owned());
    }
    tokio::fs::create_dir_all(root)
        .await
        .map_err(|e| e.to_string())
}

pub async fn list_files(app: &AppHandle, root: &str) -> Result<Vec<(String, LocalFile)>, String> {
    #[cfg(target_os = "android")]
    if root.starts_with("content://") {
        return super::android_storage::list(app, root)?
            .into_iter()
            .map(|entry| {
                LocalFile::new(root, &entry.relative_path).map(|file| (entry.relative_path, file))
            })
            .collect();
    }
    let _ = app;
    let mut files = Vec::new();
    let base = PathBuf::from(root);
    let mut directories = vec![base.clone()];
    while let Some(directory) = directories.pop() {
        let mut entries = tokio::fs::read_dir(directory)
            .await
            .map_err(|e| e.to_string())?;
        while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(".DS_Store")
            {
                continue;
            }
            let kind = entry.file_type().await.map_err(|e| e.to_string())?;
            // Do not follow links outside the selected save folder.
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file() {
                let path = entry.path();
                let relative = path
                    .strip_prefix(&base)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("세이브 파일 이름을 읽지 못했습니다.")?
                    .replace('\\', "/");
                files.push((relative.clone(), LocalFile::new(root, &relative)?));
            }
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_that_escape_the_selected_folder() {
        for path in [
            "",
            "../save.sav",
            "game/../../save.sav",
            "/save.sav",
            "game\\save.sav",
            "game//save.sav",
            "game/./save.sav",
            "game\0.sav",
        ] {
            assert!(LocalFile::new("/saves", path).is_err(), "{path}");
        }
        assert!(LocalFile::new("/saves", "게임/slot 1.sav").is_ok());
    }

    #[test]
    fn rejects_content_uris_as_desktop_filesystem_paths() {
        let file = LocalFile::new("content://provider/tree/saves", "game.sav").unwrap();
        assert!(file.filesystem_path().is_err());
    }
}
