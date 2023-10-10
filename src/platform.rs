use crate::sys::{
    ulCreateString, ulDestroyString, ulEnablePlatformFileSystem, ulEnablePlatformFontLoader,
    ulPlatformSetLogger, ulStringGetData, ulStringGetLength, ULLogLevel, ULLogger, ULString,
};
use std::{ffi::CString, sync::Mutex};

extern "C" fn logger_wrapper(log_level: ULLogLevel, msg: ULString) {
    let msg_length = unsafe { ulStringGetLength(msg) };
    let msg_data = unsafe { ulStringGetData(msg) };
    let msg_slice = unsafe { std::slice::from_raw_parts(msg_data as *const u8, msg_length) };
    let msg = String::from_utf8(msg_slice.to_vec()).unwrap();

    unsafe {
        if let Some(logger) = *GLOBAL_LOGGER.lock().unwrap() {
            logger(log_level, msg)
        }
    }
}

static mut GLOBAL_LOGGER: Mutex<Option<fn(ULLogLevel, String)>> = Mutex::new(None);

/// Does a couple of things needed to initialize ultralight.
///
/// Initializes the platform font loader and sets it as the current FontLoader.
/// Initializes the platform file system (needed for loading file:/// URLs) and sets the path to `filesys_path`
/// Initializes a default logger.
pub fn init(filesys_path: String, logger: Option<fn(ULLogLevel, String)>) {
    unsafe {
        ulEnablePlatformFontLoader();

        let filesys_path = CString::new(filesys_path).unwrap();
        let filesys_path = ulCreateString(filesys_path.as_ptr());
        ulEnablePlatformFileSystem(filesys_path);
        ulDestroyString(filesys_path);

        *GLOBAL_LOGGER.lock().unwrap() = logger;

        ulPlatformSetLogger(ULLogger {
            log_message: Some(logger_wrapper),
        });
    }
}
