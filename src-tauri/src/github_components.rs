//! Fixed upstream portable packages, verified before replacing managed program files.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy)]
struct Source {
    repo: &'static str,
    asset: &'static str,
    exe: &'static str,
}
fn source(id: &str) -> Result<Source, String> {
    match id {
        "piik-share" => Ok(Source {
            repo: "TNTcraftHIM/Piik",
            asset: "piik-app-windows-amd64.zip",
            exe: "piik-app.exe",
        }),
        "rustdesk-remote" => Ok(Source {
            repo: "rustdesk/rustdesk",
            asset: "-x86_64.exe",
            exe: "rustdesk.exe",
        }),
        _ => Err("未知官方组件".into()),
    }
}
pub fn supported(id: &str) -> bool {
    source(id).is_ok()
}
fn directory(id: &str) -> Result<PathBuf, String> {
    source(id)?;
    Ok(crate::local_app_data_dir()
        .ok_or("无法定位组件目录")?
        .join("WinToolbox/GithubComponents")
        .join(id))
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
}
#[derive(Serialize, Deserialize)]
struct Record {
    version: String,
    exe: String,
    files: Vec<String>,
    sha256: String,
}
fn relative(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(':')
        && !value.contains('\0')
        && !value.starts_with(['/', '\\'])
        && value
            .split(['/', '\\'])
            .all(|p| !p.is_empty() && p != ".." && p != ".")
}
fn is_configuration(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            ["json", "ini", "toml", "db", "sqlite", "conf"]
                .iter()
                .any(|value| ext.eq_ignore_ascii_case(value))
        })
}
fn record(id: &str) -> Result<Record, String> {
    let root = directory(id)?.join("current");
    if !crate::cleaning::no_reparse_ancestors(&root) {
        return Err("组件目录包含链接".into());
    }
    if !crate::cleaning::no_reparse_ancestors(&root.join(".winease-release.json")) {
        return Err("组件记录包含链接".into());
    }
    let r: Record = serde_json::from_slice(
        &fs::read(root.join(".winease-release.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if !relative(&r.exe) || r.files.len() > 50000 || r.files.iter().any(|p| !relative(p)) {
        return Err("组件记录无效".into());
    }
    Ok(r)
}
pub fn launch_path(id: &str) -> Option<PathBuf> {
    let r = record(id).ok()?;
    let path = directory(id).ok()?.join("current").join(r.exe);
    if path.is_file() && crate::cleaning::no_reparse_ancestors(&path) {
        Some(path)
    } else {
        None
    }
}
pub fn installed_version(id: &str) -> Option<String> {
    record(id).ok().map(|r| r.version)
}
fn validate_asset(repo: &str, asset: &Asset) -> Result<String, String> {
    if !asset
        .browser_download_url
        .starts_with(&format!("https://github.com/{repo}/releases/download/"))
        || asset.size == 0
        || asset.size > 200 * 1024 * 1024
    {
        return Err("官方资源地址或大小异常".into());
    }
    let hash = asset
        .digest
        .as_deref()
        .and_then(|v| v.strip_prefix("sha256:"))
        .ok_or("官方发布资源缺少 SHA-256，不执行下载")?;
    if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("资源校验值无效".into());
    }
    Ok(hash.to_lowercase())
}
fn latest(id: &str) -> Result<(Release, Asset, String), String> {
    let src = source(id)?;
    let root = directory(id)?;
    let script=format!("$ErrorActionPreference='Stop';$r=Invoke-RestMethod -Uri 'https://api.github.com/repos/{}/releases/latest' -Headers @{{'User-Agent'='WinEase';'Accept'='application/vnd.github+json'}} -TimeoutSec 30; $r|ConvertTo-Json -Depth 8 -Compress",src.repo);
    let mut release: Release =
        serde_json::from_str(&crate::network::runtime::run(&root, &script, 45)?)
            .map_err(|e| e.to_string())?;
    let i = release
        .assets
        .iter()
        .position(|a| {
            if id == "piik-share" {
                a.name == src.asset
            } else {
                a.name.starts_with("rustdesk-")
                    && a.name.ends_with(src.asset)
                    && !a.name.contains("sciter")
            }
        })
        .ok_or("官方发布没有匹配的 Windows x64 资源")?;
    let asset = release.assets.remove(i);
    let hash = validate_asset(src.repo, &asset)?;
    Ok((release, asset, hash))
}
pub fn update_available(id: &str) -> Result<Option<String>, String> {
    let version = installed_version(id).ok_or("未安装组件")?;
    let (r, _, _) = latest(id)?;
    Ok((r.tag_name != version).then_some(r.tag_name))
}
fn owned_tree(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
        && crate::cleaning::no_reparse_ancestors(path)
        && walkdir::WalkDir::new(path)
            .into_iter()
            .all(|e| e.is_ok_and(|e| crate::cleaning::no_reparse_ancestors(e.path())))
}
pub fn manage(id: &str, operation: &str) -> Result<crate::ToolActionResult, String> {
    let start = std::time::Instant::now();
    let src = source(id)?;
    let root = directory(id)?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    if !crate::cleaning::no_reparse_ancestors(&root) {
        return Err("组件目录包含链接".into());
    }
    let current = root.join("current");
    if operation == "uninstall" {
        let r = record(id)?;
        // Only recorded program files are removed; generated files and configuration are retained.
        for file in &r.files {
            if !crate::cleaning::no_reparse_ancestors(&current.join(file)) {
                return Err("程序文件包含链接，已停止卸载".into());
            }
        }
        for file in &r.files {
            let path = current.join(file);
            if path.is_file() && !is_configuration(&path) {
                fs::remove_file(path)
                    .map_err(|e| format!("移除程序失败，请关闭组件后重试：{e}"))?;
            }
        }
        fs::remove_file(current.join(".winease-release.json")).map_err(|e| e.to_string())?;
        return Ok(crate::build_action_result(
            "uninstall_component",
            "卸载组件",
            true,
            "程序已移除，用户配置保留。",
            "仅移除 WinEase 管理的程序文件。",
            None,
            Vec::new(),
            start,
        ));
    }
    if !matches!(operation, "install" | "repair" | "update") {
        return Err("不支持的组件操作".into());
    }
    let (release, asset, hash) = latest(id)?;
    let stage = root.join(format!("stage-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = (|| -> Result<crate::ToolActionResult, String> {
        let payload = stage.join("payload");
        fs::create_dir(&payload).map_err(|e| e.to_string())?;
        let script = format!(
            "$url='{}';$destination='{}';$payload='{}';$expected='{hash}';$expectedSize={}\n{}",
            crate::network::runtime::quote(&asset.browser_download_url),
            crate::network::runtime::quote(&stage.join("download").to_string_lossy()),
            crate::network::runtime::quote(&payload.to_string_lossy()),
            asset.size,
            include_str!("../resources/github-component-download.ps1")
        );
        crate::network::runtime::run(&root, &script, 240)?;
        if id == "rustdesk-remote" {
            fs::copy(stage.join("download"), payload.join(src.exe)).map_err(|e| e.to_string())?;
        }
        let entries = walkdir::WalkDir::new(&payload)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let files: Vec<String> = entries
            .iter()
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                e.path()
                    .strip_prefix(&payload)
                    .unwrap()
                    .to_string_lossy()
                    .into()
            })
            .collect();
        let executable = entries
            .iter()
            .find(|e| {
                e.file_type().is_file()
                    && e.file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(src.exe)
            })
            .ok_or("压缩包中没有官方启动程序，不替换当前组件")?
            .path();
        use std::io::Read;
        let mut magic = [0; 2];
        fs::File::open(executable)
            .map_err(|e| e.to_string())?
            .read_exact(&mut magic)
            .map_err(|e| e.to_string())?;
        if &magic != b"MZ" {
            return Err("启动程序格式校验失败".into());
        }
        let exe = executable
            .strip_prefix(&payload)
            .unwrap()
            .to_string_lossy()
            .into();
        let old_files: HashSet<String> = record(id)
            .ok()
            .map(|r| r.files.into_iter().collect())
            .unwrap_or_default();
        if current.exists() {
            if !owned_tree(&current, &root) {
                return Err("原组件目录包含链接或无法读取，不执行更新".into());
            }
            let mut copied = 0u64;
            for e in walkdir::WalkDir::new(&current).into_iter() {
                let e = e.map_err(|e| e.to_string())?;
                if !e.file_type().is_file() {
                    continue;
                }
                let rel = e.path().strip_prefix(&current).unwrap();
                let name = rel.to_string_lossy();
                if name == ".winease-release.json" {
                    continue;
                }
                let configuration = is_configuration(e.path());
                if (!old_files.contains(name.as_ref()) || configuration)
                    && (!files.contains(&name.to_string()) || configuration)
                {
                    copied = copied.saturating_add(e.metadata().map_err(|e| e.to_string())?.len());
                    if copied > 512 * 1024 * 1024 {
                        return Err("用户数据超过迁移上限；原版本保留，请先整理组件目录".into());
                    }
                    let target = payload.join(rel);
                    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                    fs::copy(e.path(), target)
                        .map_err(|e| format!("配置迁移失败，请先关闭组件：{e}"))?;
                }
            }
        }
        fs::write(
            payload.join(".winease-release.json"),
            serde_json::to_vec_pretty(&Record {
                version: release.tag_name,
                exe,
                files,
                sha256: hash,
            })
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let backup = root.join(format!("backup-{}", uuid::Uuid::new_v4()));
        if current.exists() {
            fs::rename(&current, &backup)
                .map_err(|e| format!("组件正在使用，请关闭后重试：{e}"))?;
        }
        if let Err(error) = fs::rename(&payload, &current) {
            if backup.exists() {
                fs::rename(&backup, &current)
                    .map_err(|e| format!("切换失败且回退未完成：{error}；{e}"))?;
            }
            return Err(error.to_string());
        }
        let mut warnings = Vec::new();
        if backup.exists() && owned_tree(&backup, &root) {
            if fs::remove_dir_all(&backup).is_err() {
                warnings.push("旧程序目录仍在使用，已保留。".into());
            }
        }
        Ok(crate::build_action_result(
            "install_component",
            "官方组件",
            true,
            "组件已准备完成，可以打开。",
            "来源：作者 GitHub 正式发布；SHA-256 校验通过。未开启远程服务或自动分享。",
            None,
            warnings,
            start,
        ))
    })();
    if stage.exists() && owned_tree(&stage, &root) {
        let _ = fs::remove_dir_all(stage);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unreviewed_sources_paths_and_missing_digests_are_rejected() {
        assert!(!supported("https://evil"));
        for p in [
            "../file",
            "C:\\file",
            "\\\\server\\file",
            "folder/file:stream",
        ] {
            assert!(!relative(p));
        }
        assert!(relative("folder/app.exe"));
        let a = Asset {
            name: "app.exe".into(),
            browser_download_url: "https://evil/app.exe".into(),
            digest: Some(format!("sha256:{}", "a".repeat(64))),
            size: 10,
        };
        assert!(validate_asset("rustdesk/rustdesk", &a).is_err());
        let mut a = Asset {
            browser_download_url:
                "https://github.com/rustdesk/rustdesk/releases/download/1.5.0/rustdesk-x86_64.exe"
                    .into(),
            ..a
        };
        assert!(validate_asset("rustdesk/rustdesk", &a).is_ok());
        a.digest = None;
        assert!(validate_asset("rustdesk/rustdesk", &a).is_err());
        a.digest = Some("sha256:invalid".into());
        assert!(validate_asset("rustdesk/rustdesk", &a).is_err());
        a.digest = Some(format!("sha256:{}", "a".repeat(64)));
        a.size = 201 * 1024 * 1024;
        assert!(validate_asset("rustdesk/rustdesk", &a).is_err());
        assert!(is_configuration(Path::new("profile.JSON")));
        assert!(!is_configuration(Path::new("program.exe")));
    }
    #[test]
    fn archive_entries_cannot_escape_the_component_directory() {
        let root = tempfile::tempdir().unwrap();
        let zip = root.path().join("fixture.zip");
        let payload = root.path().join("payload");
        fs::create_dir(&payload).unwrap();
        let guard = include_str!("../resources/github-component-download.ps1")
            .split("if($url.EndsWith('.zip'))")
            .nth(1)
            .unwrap();
        let script=format!("$ErrorActionPreference='Stop';Add-Type -AssemblyName System.IO.Compression.FileSystem;$destination='{}';$payload='{}';$url='https://example.invalid/fixture.zip';$z=[IO.Compression.ZipFile]::Open($destination,'Create');$e=$z.CreateEntry('../escaped.exe');$stream=$e.Open();$stream.WriteByte(1);$stream.Dispose();$z.Dispose();if($url.EndsWith('.zip')){guard}",crate::network::runtime::quote(&zip.to_string_lossy()),crate::network::runtime::quote(&payload.to_string_lossy()));
        let result = crate::network::runtime::run(root.path(), &script, 20);
        assert!(result.unwrap_err().contains("越界"));
        assert!(!root.path().join("escaped.exe").exists());
    }
    #[test]
    fn ordinary_archive_keeps_program_and_license_files() {
        let root = tempfile::tempdir().unwrap();
        let zip = root.path().join("fixture.zip");
        let payload = root.path().join("payload");
        fs::create_dir(&payload).unwrap();
        let guard = include_str!("../resources/github-component-download.ps1")
            .split("if($url.EndsWith('.zip'))")
            .nth(1)
            .unwrap();
        let script=format!("$ErrorActionPreference='Stop';Add-Type -AssemblyName System.IO.Compression.FileSystem;$destination='{}';$payload='{}';$url='https://example.invalid/fixture.zip';$z=[IO.Compression.ZipFile]::Open($destination,'Create');foreach($name in @('app.exe','licenses/LICENSE')){{$e=$z.CreateEntry($name);$stream=$e.Open();$stream.WriteByte(1);$stream.Dispose()}};$z.Dispose();if($url.EndsWith('.zip')){guard}",crate::network::runtime::quote(&zip.to_string_lossy()),crate::network::runtime::quote(&payload.to_string_lossy()));
        crate::network::runtime::run(root.path(), &script, 20).unwrap();
        assert!(payload.join("app.exe").is_file());
        assert!(payload.join("licenses/LICENSE").is_file());
    }
    #[test]
    #[ignore = "read-only upstream release metadata; never installs or opens apps"]
    fn upstream_release_metadata_is_readable() {
        for id in ["piik-share", "rustdesk-remote"] {
            let (r, a, hash) = latest(id).unwrap();
            println!("{id}: {} {} {} bytes", r.tag_name, a.name, a.size);
            assert_eq!(hash.len(), 64);
        }
    }
    #[test]
    #[ignore = "download and inspect official Piik zip in a temporary directory; never installs or executes"]
    fn official_piik_archive_contains_expected_launcher() {
        let (_, asset, hash) = latest("piik-share").unwrap();
        let root = tempfile::tempdir().unwrap();
        let payload = root.path().join("payload");
        fs::create_dir(&payload).unwrap();
        let script = format!(
            "$url='{}';$destination='{}';$payload='{}';$expected='{hash}';$expectedSize={}\n{}",
            crate::network::runtime::quote(&asset.browser_download_url),
            crate::network::runtime::quote(&root.path().join("download").to_string_lossy()),
            crate::network::runtime::quote(&payload.to_string_lossy()),
            asset.size,
            include_str!("../resources/github-component-download.ps1")
        );
        crate::network::runtime::run(root.path(), &script, 240).unwrap();
        let entries = walkdir::WalkDir::new(&payload)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(entries.iter().any(|e| e
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case("piik-app.exe")));
        println!(
            "Piik archive verified; {} entries inspected; no application executed",
            entries.len()
        );
    }
}
