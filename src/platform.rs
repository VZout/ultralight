use crate::sys::{
    ulCreateString, ulDestroyString, ulEnablePlatformFileSystem, ulEnablePlatformFontLoader,
    ulPlatformSetLogger, ulStringGetData, ulStringGetLength, ULLogLevel,
    ULLogLevel_kLogLevel_Error, ULLogLevel_kLogLevel_Info, ULLogLevel_kLogLevel_Warning, ULLogger,
    ULString,
};
use std::ffi::CString;

extern "C" fn default_logger(log_level: ULLogLevel, msg: ULString) {
    let msg_length = unsafe { ulStringGetLength(msg) };
    let msg_data = unsafe { ulStringGetData(msg) };
    let msg_slice = unsafe { std::slice::from_raw_parts(msg_data as *const u8, msg_length) };
    let msg = String::from_utf8(msg_slice.to_vec()).unwrap();

    if log_level == ULLogLevel_kLogLevel_Info {
        println!("[ultralight]: {}", msg);
    } else if log_level == ULLogLevel_kLogLevel_Warning {
        eprintln!("[ultralight][warn]: {}", msg);
    } else if log_level == ULLogLevel_kLogLevel_Error {
        eprintln!("[ultralight][error]: {}", msg);
    }
}

/// Does a couple of things needed to initialize ultralight.
///
/// Initializes the platform font loader and sets it as the current FontLoader.
/// Initializes the platform file system (needed for loading file:/// URLs) and sets the path to `filesys_path`
/// Initializes a default logger.
pub fn init(filesys_path: String) {
    unsafe {
        ulEnablePlatformFontLoader();

        let filesys_path = CString::new(filesys_path).unwrap();
        let filesys_path = ulCreateString(filesys_path.as_ptr());
        ulEnablePlatformFileSystem(filesys_path);
        ulDestroyString(filesys_path);

        ulPlatformSetLogger(ULLogger {
            log_message: Some(default_logger),
        });
    }
}
