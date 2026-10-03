use std::{
    os::windows::ffi::OsStrExt,
    path::{Component, Path, Prefix},
};
use windows::{
    core::PCWSTR,
    Win32::{System::Com::*, UI::Shell::*},
};

struct ComGuard;
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
// Force recycling; never retry with a permanent delete when the volume refuses it.
pub fn recycle(path: &Path) -> Result<(), String> {
    let prefix = match path.components().next() {
        Some(Component::Prefix(p)) => p.kind(),
        _ => return Err("文件路径无效".into()),
    };
    let skip = match prefix {
        Prefix::VerbatimDisk(_) => 4,
        Prefix::Disk(_) => 0,
        _ => return Err("此位置不支持安全回收".into()),
    };
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .skip(skip)
        .chain(Some(0))
        .collect();
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
        let _guard = ComGuard;
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        operation
            .SetOperationFlags(
                FOF_NO_UI
                    | FOFX_RECYCLEONDELETE
                    | FOFX_ADDUNDORECORD
                    | FOFX_EARLYFAILURE
                    | FOF_WANTNUKEWARNING,
            )
            .map_err(|e| e.to_string())?;
        let item: IShellItem =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).map_err(|e| e.to_string())?;
        operation
            .DeleteItem(&item, None)
            .map_err(|e| e.to_string())?;
        operation.PerformOperations().map_err(|e| e.to_string())?;
        if operation
            .GetAnyOperationsAborted()
            .map_err(|e| e.to_string())?
            .as_bool()
        {
            return Err("Windows 未完成回收操作".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_recycle_can_be_restored_without_touching_other_items() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join(format!("toolbox-recycle-test-{}", std::process::id()));
        std::fs::write(&path, b"recoverable test file").unwrap();
        let canonical = std::fs::canonicalize(&path).unwrap();
        recycle(&canonical).unwrap();
        assert!(!path.exists());
        let items = trash::os_limited::list()
            .unwrap()
            .into_iter()
            .filter(|item| item.name == path.file_name().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(items.len(), 1, "test file must be in the recycle bin");
        trash::os_limited::restore_all(items).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"recoverable test file");
    }
}
