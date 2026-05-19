// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ptr;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK, MB_ICONERROR};

fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = 0;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut size = 0u32;
        let result = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        CloseHandle(token);
        result != 0 && elevation.TokenIsElevated != 0
    }
}

fn is_already_running() -> bool {
    unsafe {
        let name: Vec<u16> = "Global\\VLanGaming_SingleInstance\0".encode_utf16().collect();
        let handle = CreateMutexW(ptr::null(), 1, name.as_ptr());
        if handle == 0 {
            return true;
        }
        // ERROR_ALREADY_EXISTS = 183
        GetLastError() == 183
    }
}

fn show_error(msg: &str) {
    unsafe {
        let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
        let title: Vec<u16> = "VLan Gaming\0".encode_utf16().collect();
        MessageBoxW(0, wide.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
    }
}

fn main() {
    if !is_elevated() {
        show_error("请以管理员身份运行此程序，否则无法创建虚拟网卡。");
        return;
    }

    if is_already_running() {
        show_error("VLan Gaming 已在运行中，请勿重复启动。");
        return;
    }

    client_tauri_lib::run()
}
