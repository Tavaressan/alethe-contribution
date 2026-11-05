#[cfg(target_os = "linux")]
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASES_DOWNLOAD_URL: &str = "https://github.com/Kc1t/alethe-agents/releases/download";

/// Distro packages install this file to say who updates the app. It declares ownership and is not
/// a security boundary: repackaged binaries keep their bundle type, so the updater cannot tell.
#[cfg(target_os = "linux")]
const MANAGED_BY_PATH: &str = "/usr/share/alethe/managed-by";

#[derive(Default)]
pub struct PendingRelease(Mutex<Option<(Update, Vec<u8>)>>);

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all_fields = "camelCase")]
pub enum ReleaseDownloadEvent {
    Started {
        #[serde(skip_serializing_if = "Option::is_none")]
        content_length: Option<u64>,
    },
    Progress {
        chunk_length: usize,
    },
    Finished,
}

fn is_release_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 5 && part.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn release_manifest_url(version: &str) -> Result<Url, String> {
    if !is_release_version(version) {
        return Err("invalid_release_version".to_string());
    }
    Url::parse(&format!("{RELEASES_DOWNLOAD_URL}/v{version}/latest.json"))
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn manager_name(contents: &str) -> String {
    let name = contents.lines().next().unwrap_or_default().trim();
    let valid = name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c));
    if valid {
        name.to_string()
    } else {
        String::new()
    }
}

#[cfg(target_os = "linux")]
fn managed_by_marker(marker: &Path, in_appimage: bool) -> Option<String> {
    // An AppImage updates itself, even on a machine that also has the distro package installed.
    if in_appimage || !marker.exists() {
        return None;
    }
    Some(manager_name(
        &std::fs::read_to_string(marker).unwrap_or_default(),
    ))
}

/// The package manager that owns this install. An empty name means the marker names none.
pub fn managed_by() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        managed_by_marker(
            Path::new(MANAGED_BY_PATH),
            std::env::var_os("APPIMAGE").is_some(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[tauri::command]
pub fn package_manager() -> Option<String> {
    managed_by()
}

#[tauri::command]
pub async fn download_release(
    app: AppHandle,
    pending: State<'_, PendingRelease>,
    version: String,
    on_event: Channel<ReleaseDownloadEvent>,
) -> Result<(), String> {
    if managed_by().is_some() {
        return Err("managed_install".to_string());
    }
    let endpoint = release_manifest_url(&version)?;
    let wanted = version.clone();
    let update = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|error| error.to_string())?
        .version_comparator(move |current, release| {
            release.version != current && release.version.to_string() == wanted
        })
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "release_not_installable".to_string())?;

    let progress = on_event.clone();
    let mut started = false;
    let bytes = update
        .download(
            move |chunk_length, content_length| {
                if !started {
                    started = true;
                    let _ = progress.send(ReleaseDownloadEvent::Started { content_length });
                }
                let _ = progress.send(ReleaseDownloadEvent::Progress { chunk_length });
            },
            move || {
                let _ = on_event.send(ReleaseDownloadEvent::Finished);
            },
        )
        .await
        .map_err(|error| error.to_string())?;

    *pending
        .0
        .lock()
        .map_err(|_| "release_state_poisoned".to_string())? = Some((update, bytes));
    Ok(())
}

#[tauri::command]
pub fn install_downloaded_release(pending: State<'_, PendingRelease>) -> Result<(), String> {
    let (update, bytes) = pending
        .0
        .lock()
        .map_err(|_| "release_state_poisoned".to_string())?
        .take()
        .ok_or_else(|| "no_release_downloaded".to_string())?;
    update.install(bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{is_release_version, release_manifest_url, ReleaseDownloadEvent};

    #[test]
    fn only_plain_release_versions_build_a_manifest_url() {
        assert_eq!(
            release_manifest_url("1.6.0").unwrap().as_str(),
            "https://github.com/Kc1t/alethe-agents/releases/download/v1.6.0/latest.json"
        );
        for version in [
            "",
            "1.6",
            "1.6.0.1",
            "v1.6.0",
            "1.6.0-beta",
            "1.6.0/../x",
            "1..0",
            "1.6.0?x=1",
            "123456.0.0",
        ] {
            assert!(!is_release_version(version), "{version}");
            assert!(release_manifest_url(version).is_err(), "{version}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reads_the_manager_name_from_the_marker() {
        use super::manager_name;

        assert_eq!(manager_name("pacman\n"), "pacman");
        assert_eq!(manager_name("  nix-env  \nignored"), "nix-env");
        assert_eq!(manager_name(""), "");
        assert_eq!(manager_name("not a name"), "");
        assert_eq!(manager_name(&"a".repeat(65)), "");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_marker_disables_updates_except_inside_an_appimage() {
        use super::managed_by_marker;

        let dir = std::env::temp_dir().join(format!("alethe-marker-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("managed-by");

        assert_eq!(managed_by_marker(&marker, false), None);

        std::fs::write(&marker, "pacman\n").unwrap();
        assert_eq!(
            managed_by_marker(&marker, false),
            Some("pacman".to_string())
        );
        assert_eq!(managed_by_marker(&marker, true), None);

        std::fs::write(&marker, "").unwrap();
        assert_eq!(managed_by_marker(&marker, false), Some(String::new()));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn download_events_match_the_updater_plugin_shape() {
        let started = ReleaseDownloadEvent::Started {
            content_length: Some(42),
        };
        let unknown = ReleaseDownloadEvent::Started {
            content_length: None,
        };
        let progress = ReleaseDownloadEvent::Progress { chunk_length: 7 };
        assert_eq!(
            serde_json::to_value(started).unwrap(),
            serde_json::json!({ "event": "Started", "data": { "contentLength": 42 } })
        );
        assert_eq!(
            serde_json::to_value(unknown).unwrap(),
            serde_json::json!({ "event": "Started", "data": {} })
        );
        assert_eq!(
            serde_json::to_value(progress).unwrap(),
            serde_json::json!({ "event": "Progress", "data": { "chunkLength": 7 } })
        );
        assert_eq!(
            serde_json::to_value(ReleaseDownloadEvent::Finished).unwrap(),
            serde_json::json!({ "event": "Finished" })
        );
    }
}
