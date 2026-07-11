//! 本文件将 sqlite-vec 的唯一不安全 FFI 注册点封装为幂等安全接口。

#![allow(unsafe_code)]
#![deny(missing_docs)]

use std::sync::OnceLock;

type ExtensionEntry = unsafe extern "C" fn(
    *mut rusqlite::ffi::sqlite3,
    *mut *mut std::ffi::c_char,
    *const rusqlite::ffi::sqlite3_api_routines,
) -> std::ffi::c_int;

static REGISTRATION: OnceLock<Result<(), i32>> = OnceLock::new();

/// 为当前进程后续创建的 SQLite 连接注册 sqlite-vec 扩展。
pub fn register() -> Result<(), i32> {
    REGISTRATION
        .get_or_init(|| {
            // sqlite-vec 官方 Rust 绑定只暴露 C 初始化符号；SQLite 要求把它转换为自动扩展回调。
            let result = unsafe {
                rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                    *const (),
                    ExtensionEntry,
                >(
                    sqlite_vec::sqlite3_vec_init as *const (),
                )))
            };
            if result == rusqlite::ffi::SQLITE_OK {
                Ok(())
            } else {
                Err(result)
            }
        })
        .to_owned()
}
