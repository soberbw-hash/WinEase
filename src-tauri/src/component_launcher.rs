//! GUI component launches: use ordinary permissions unless Windows requires elevation.
use std::{
    io,
    os::windows::{ffi::OsStrExt, process::CommandExt},
    path::Path,
    process::{Command, Stdio},
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::CloseHandle,
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
        UI::{
            Shell::{
                ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
                SHELLEXECUTEINFOW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
};

pub fn launch(executable: &Path, args: &[String]) -> Result<(), String> {
    // Reject embedded NULs before either path, so ShellExecute cannot silently truncate them.
    let file = wide(executable.as_os_str().encode_wide())?;
    let parameters = wide(
        args.iter()
            .map(|arg| quote_argument(arg))
            .collect::<Vec<_>>()
            .join(" ")
            .encode_utf16(),
    )?;
    let directory = executable
        .parent()
        .filter(|path| !path.as_os_str().is_empty());
    let directory_wide = directory
        .map(|path| wide(path.as_os_str().encode_wide()))
        .transpose()?;
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(super::CREATE_NO_WINDOW);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    finish_launch(command.spawn().map(|_| ()), || {
        launch_elevated(&file, &parameters, directory_wide.as_deref())
    })
    .map_err(|error| format!("无法打开 {}：{error}", executable.display()))
}

fn finish_launch(
    result: io::Result<()>,
    elevate: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(740) => elevate(),
        Err(error) => Err(error.to_string()),
    }
}

fn launch_elevated(
    file: &[u16],
    parameters: &[u16],
    directory: Option<&[u16]>,
) -> Result<(), String> {
    // Tauri's blocking worker has no message loop. Complete Shell launch before returning,
    // but do not wait for the component to exit. Balance COM only when this call initialized it.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        lpDirectory: directory.map_or(PCWSTR::null(), |value| PCWSTR(value.as_ptr())),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // All UTF-16 buffers stay alive until the synchronous API returns.
    let result = unsafe { ShellExecuteExW(&mut info) };
    if !info.hProcess.is_invalid() {
        unsafe {
            let _ = CloseHandle(info.hProcess);
        }
    }
    if initialized {
        unsafe {
            CoUninitialize();
        }
    }
    result.map_err(|error| shell_error_message(error.code().0 as u32 & 0xffff, &error.to_string()))
}

fn shell_error_message(code: u32, details: &str) -> String {
    if code == 1223 {
        "已取消管理员授权，组件未打开。".into()
    } else {
        format!("Windows 启动失败（错误 {code}）：{details}")
    }
}

fn wide(value: impl Iterator<Item = u16>) -> Result<Vec<u16>, String> {
    let mut value: Vec<_> = value.collect();
    if value.contains(&0) {
        return Err("启动路径或参数包含无效字符。".into());
    }
    value.push(0);
    Ok(value)
}

// Windows argv quoting, not shell/script escaping. Preserve empty arguments, embedded
// quotes and backslashes before a quote or the closing quote.
fn quote_argument(arg: &str) -> String {
    let mut quoted = String::from("\"");
    let mut slashes = 0;
    for character in arg.chars() {
        if character == '\\' {
            slashes += 1;
            continue;
        }
        quoted.extend(std::iter::repeat_n(
            '\\',
            if character == '"' {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        slashes = 0;
        quoted.push(character);
    }
    quoted.extend(std::iter::repeat_n('\\', slashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_success_never_requests_elevation() {
        assert!(finish_launch(Ok(()), || panic!("unexpected elevation")).is_ok());
    }
    #[test]
    fn elevation_required_uses_fallback_and_propagates_cancellation() {
        let message = finish_launch(Err(io::Error::from_raw_os_error(740)), || {
            Err(shell_error_message(1223, "cancelled"))
        })
        .unwrap_err();
        assert!(message.contains("已取消管理员授权"));
    }
    #[test]
    fn missing_file_or_access_denied_never_requests_elevation() {
        for code in [2, 3, 5, 193] {
            assert!(
                finish_launch(Err(io::Error::from_raw_os_error(code)), || panic!(
                    "unexpected elevation"
                ))
                .is_err()
            );
        }
    }
    #[test]
    fn rejects_nul_in_arguments() {
        assert!(launch(Path::new("unused.exe"), &["bad\0argument".into()]).is_err());
    }
    #[test]
    fn windows_parser_roundtrips_parameters() {
        use windows::Win32::{
            Foundation::{LocalFree, HLOCAL},
            UI::Shell::CommandLineToArgvW,
        };
        let args = [
            "",
            "plain",
            "中文 路径",
            r"C:\Program Files\tool\",
            "a\"b",
            "slash\\\"quote",
            "& $() ;",
        ];
        let command = format!("component.exe {}", args.map(quote_argument).join(" "));
        let command = wide(command.encode_utf16()).unwrap();
        let mut count = 0;
        unsafe {
            let parsed = CommandLineToArgvW(PCWSTR(command.as_ptr()), &mut count);
            assert!(!parsed.is_null());
            let actual: Vec<String> = std::slice::from_raw_parts(parsed, count as usize)
                .iter()
                .skip(1)
                .map(|arg| arg.to_string().unwrap())
                .collect();
            let _ = LocalFree(Some(HLOCAL(parsed.cast())));
            assert_eq!(actual, args);
        }
    }
}
