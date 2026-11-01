use std::{fs::File, os::fd::FromRawFd};

use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    AppHandle, Manager,
};

struct AndroidStorage(PluginHandle<tauri::Wry>);

pub fn init() -> TauriPlugin<tauri::Wry> {
    Builder::<tauri::Wry, ()>::new("save-storage")
        .setup(|app, api| {
            let handle =
                api.register_android_plugin("com.garoad.cloudgamesaver", "SaveStoragePlugin")?;
            app.manage(AndroidStorage(handle));
            Ok(())
        })
        .build()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileArgs<'a> {
    tree_uri: &'a str,
    relative_path: &'a str,
    mode: &'a str,
}

#[derive(Deserialize)]
struct TreeSelection {
    uri: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub exists: bool,
    #[serde(default)]
    pub modified_millis: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub relative_path: String,
}

#[derive(Deserialize)]
struct FileList {
    files: Vec<FileEntry>,
}

#[derive(Deserialize)]
struct FileDescriptor {
    fd: i32,
}

pub fn pick_folder(app: &AppHandle) -> Result<Option<String>, String> {
    app.state::<AndroidStorage>()
        .0
        .run_mobile_plugin::<TreeSelection>("pickTree", ())
        .map(|result| result.uri)
        .map_err(|e| e.to_string())
}

pub fn validate_tree(app: &AppHandle, tree_uri: &str) -> Result<(), String> {
    app.state::<AndroidStorage>()
        .0
        .run_mobile_plugin::<serde_json::Value>(
            "validateTree",
            FileArgs {
                tree_uri,
                relative_path: "",
                mode: "",
            },
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn list(app: &AppHandle, tree_uri: &str) -> Result<Vec<FileEntry>, String> {
    app.state::<AndroidStorage>()
        .0
        .run_mobile_plugin::<FileList>(
            "listFiles",
            FileArgs {
                tree_uri,
                relative_path: "",
                mode: "",
            },
        )
        .map(|result| result.files)
        .map_err(|e| e.to_string())
}

pub fn stat(app: &AppHandle, tree_uri: &str, relative_path: &str) -> Result<FileStat, String> {
    app.state::<AndroidStorage>()
        .0
        .run_mobile_plugin(
            "statFile",
            FileArgs {
                tree_uri,
                relative_path,
                mode: "",
            },
        )
        .map_err(|e| e.to_string())
}

pub fn open(
    app: &AppHandle,
    tree_uri: &str,
    relative_path: &str,
    mode: &str,
) -> Result<File, String> {
    let response: FileDescriptor = app
        .state::<AndroidStorage>()
        .0
        .run_mobile_plugin(
            "openFile",
            FileArgs {
                tree_uri,
                relative_path,
                mode,
            },
        )
        .map_err(|e| e.to_string())?;
    if response.fd < 0 {
        return Err("세이브 파일을 열지 못했습니다.".to_owned());
    }
    // Kotlin detaches the ParcelFileDescriptor. This File takes sole ownership
    // of the descriptor in the shared Android process and closes it on drop.
    Ok(unsafe { File::from_raw_fd(response.fd) })
}
