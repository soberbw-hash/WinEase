use base64::{engine::general_purpose::STANDARD, Engine};
use std::{collections::VecDeque, path::PathBuf, sync::Mutex, time::SystemTime};

type CachedIcon = (String, u64, Option<SystemTime>, Option<String>);
static CACHE: Mutex<VecDeque<CachedIcon>> = Mutex::new(VecDeque::new());

fn expand_environment(value: &str) -> Option<String> {
    if value.len() > 4096 || value.contains('\0') {
        return None;
    }
    let mut result = String::new();
    let mut remaining = value;
    while let Some(start) = remaining.find('%') {
        result.push_str(&remaining[..start]);
        let tail = &remaining[start + 1..];
        let end = tail.find('%')?;
        result.push_str(&std::env::var(&tail[..end]).ok()?);
        remaining = &tail[end + 1..];
    }
    result.push_str(remaining);
    (result.len() <= 4096).then_some(result)
}

fn local_application_path(value: &str) -> Option<PathBuf> {
    let bytes = value.as_bytes();
    // Only local drive paths. Do not query URLs, device paths or network shares.
    if bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'\\' | b'/')
    {
        return None;
    }
    let path = PathBuf::from(value);
    let extension = path.extension()?.to_str()?;
    if !["exe", "lnk", "ico"]
        .iter()
        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
    {
        return None;
    }
    if !crate::cleaning::no_reparse_ancestors(&path) || !path.is_file() {
        return None;
    }
    Some(path)
}

fn resolve_target(target: &str, command: bool) -> Option<PathBuf> {
    let expanded = expand_environment(target)?;
    let value = expanded.trim();
    if !command {
        return local_application_path(value);
    }
    if let Some(quoted) = value.strip_prefix('"') {
        return local_application_path(&quoted[..quoted.find('"')?]);
    }
    // Resolve the executable portion only; never start a command or parse its arguments as paths.
    if let Some(first) = value
        .split_whitespace()
        .next()
        .and_then(local_application_path)
    {
        return Some(first);
    }
    let lower = value.to_ascii_lowercase();
    for (index, _) in lower.match_indices(".exe") {
        let end = index + 4;
        if end == value.len() || value[end..].starts_with(char::is_whitespace) {
            if let Some(path) = local_application_path(&value[..end]) {
                return Some(path);
            }
        }
    }
    None
}

fn load(target: &str, command: bool) -> Option<String> {
    let path = resolve_target(target, command)?;
    let metadata = path.metadata().ok()?;
    let key = path.to_string_lossy().to_lowercase();
    let modified = metadata.modified().ok();
    let len = metadata.len();
    if let Some(cached) = CACHE
        .lock()
        .ok()?
        .iter()
        .find(|item| item.0 == key && item.1 == len && item.2 == modified)
    {
        return cached.3.clone();
    }
    let image = (|| {
        let mut icon = file_icon_provider::get_file_icon(&path, 32).ok()?;
        if icon.width != 32 || icon.height != 32 || icon.pixels.len() != 32 * 32 * 4 {
            return None;
        }
        // Windows Shell returns premultiplied RGBA; PNG expects straight alpha.
        for pixel in icon.pixels.chunks_exact_mut(4) {
            let alpha = pixel[3] as u32;
            if alpha > 0 && alpha < 255 {
                for channel in &mut pixel[..3] {
                    *channel = ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
        }
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 32, 32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .ok()?
                .write_image_data(&icon.pixels)
                .ok()?;
        }
        Some(format!("data:image/png;base64,{}", STANDARD.encode(png)))
    })();
    let mut cache = CACHE.lock().ok()?;
    cache.retain(|item| item.0 != key);
    if cache.len() >= 256 {
        cache.pop_front();
    }
    cache.push_back((key, len, modified, image.clone()));
    image
}

#[tauri::command]
pub async fn get_application_icon(target: String, command: bool) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || load(&target, command))
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_resolution_ignores_arguments_and_accepts_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("应用 name.exe");
        std::fs::write(&exe, b"fixture").unwrap();
        let path = exe.to_string_lossy();
        assert_eq!(
            resolve_target(
                &format!("\"{path}\" --url https://example.com/file.exe"),
                true
            ),
            Some(exe.clone())
        );
        assert_eq!(
            resolve_target(&format!("{path} --argument"), true),
            Some(exe.clone())
        );
        assert!(resolve_target(&format!("\"{path}"), true).is_none());
    }
    #[test]
    fn remote_and_script_targets_are_rejected_before_file_access() {
        for value in [
            r"\\server\share\app.exe",
            "https://example.com/app.exe",
            r"\\?\C:\app.exe",
            r"C:\app.ps1",
            "C:\\app.exe\0",
        ] {
            assert!(resolve_target(value, false).is_none());
        }
        assert!(resolve_target("%WINEASE_MISSING_ICON_ENVIRONMENT%\\app.exe", false).is_none());
    }
    #[test]
    fn windows_icon_is_a_valid_png_and_reuses_cache() {
        let path = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("explorer.exe");
        let first = load(&path.to_string_lossy(), false).expect("Explorer icon");
        assert_eq!(Some(first.clone()), load(&path.to_string_lossy(), false));
        let bytes = STANDARD
            .decode(first.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        let reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        assert_eq!((reader.info().width, reader.info().height), (32, 32));
    }
}
