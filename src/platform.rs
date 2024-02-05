#[cfg(feature = "filewatching")]
use notify::{Error, Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::sys::{
    ulCreateBuffer, ulCreateString, ulDestroyString, ulEnablePlatformFileSystem,
    ulEnablePlatformFontLoader, ulPlatformSetFileSystem, ulPlatformSetLogger, ulStringGetData,
    ulStringGetLength, C_String, ULBuffer, ULFileSystem, ULLogLevel, ULLogger, ULString,
};
use std::{
    ffi::CString,
    io::Read,
    os::raw::c_void,
    sync::{Mutex, RwLock},
};

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
static mut BASE_ASSET_DIR: RwLock<String> = RwLock::new(String::new());

#[cfg(feature = "filewatching")]
static mut WATCHER: Mutex<Option<RecommendedWatcher>> = Mutex::new(None);
#[cfg(feature = "filewatching")]
pub(crate) static mut ASSETS_MODIFIED: RwLock<bool> = RwLock::new(false);

/// Does a couple of things needed to initialize ultralight.
///
/// Initializes the platform font loader and sets it as the current FontLoader.
/// Initializes the platform file system (needed for loading file:/// URLs) and sets the path to `filesys_path`
/// Initializes a default logger.
pub fn init(filesys_path: String, logger: Option<fn(ULLogLevel, String)>) {
    unsafe {
        ulEnablePlatformFontLoader();

        if true {
            *BASE_ASSET_DIR.write().unwrap() = filesys_path.clone();
            ulPlatformSetFileSystem(ULFileSystem {
                file_exists: Some(file_exists),
                get_file_mime_type: Some(file_mime_type),
                get_file_charset: Some(file_charset),
                open_file: Some(open_file),
            });
        } else {
            let filesys_path = CString::new(filesys_path.clone()).unwrap();
            let filesys_path = ulCreateString(filesys_path.as_ptr());
            ulEnablePlatformFileSystem(filesys_path);
            ulDestroyString(filesys_path);
        }

        #[cfg(feature = "filewatching")]
        init_filewatcher(&filesys_path);

        *GLOBAL_LOGGER.lock().unwrap() = logger;

        ulPlatformSetLogger(ULLogger {
            log_message: Some(logger_wrapper),
        });
    }
}

#[cfg(feature = "filewatching")]
fn init_filewatcher(asset_dir: &str) {
    let asset_dir = std::path::Path::new(asset_dir);

    let mut watcher = notify::recommended_watcher(|res: Result<Event, Error>| match res {
        Ok(event) => {
            if event.kind.is_modify() {
                *unsafe { ASSETS_MODIFIED.write().unwrap() } = true
            }
        }
        Err(e) => println!("file watch error: {:?}", e),
    })
    .unwrap();
    watcher.watch(asset_dir, RecursiveMode::Recursive).unwrap();

    *unsafe { WATCHER.lock().unwrap() } = Some(watcher);
}

pub fn assets_modified() -> bool {
    #[cfg(feature = "filewatching")]
    unsafe {
        *ASSETS_MODIFIED.read().unwrap()
    }

    #[cfg(not(feature = "filewatching"))]
    false
}

fn read_ulstring(input: ULString) -> String {
    let msg_length = unsafe { ulStringGetLength(input) };
    let msg_data = unsafe { ulStringGetData(input) };
    let msg_slice = unsafe { std::slice::from_raw_parts(msg_data as *const u8, msg_length) };
    String::from_utf8(msg_slice.to_vec()).unwrap()
}

unsafe extern "C" fn file_exists(path: *mut C_String) -> bool {
    let path = format!("{}/{}", BASE_ASSET_DIR.read().unwrap(), read_ulstring(path));
    std::path::Path::new(&path).exists()
}

unsafe extern "C" fn file_mime_type(path: *mut C_String) -> *mut C_String {
    let path = format!("{}/{}", BASE_ASSET_DIR.read().unwrap(), read_ulstring(path));
    let guess = mime_guess::from_path(path);

    let mime = guess
        .first()
        .map(|mime| mime.to_string())
        .unwrap_or(String::from("application/unknown"));
    let mime = CString::new(mime).unwrap();

    ulCreateString(mime.as_ptr()) // Destroyed by ultralight
}

unsafe extern "C" fn file_charset(_: *mut C_String) -> ULString {
    let charset = CString::new("utf-8").unwrap();
    ulCreateString(charset.as_ptr()) // Destroyed by ultralight
}

unsafe extern "C" fn open_file(path: *mut C_String) -> ULBuffer {
    let path = format!("{}/{}", BASE_ASSET_DIR.read().unwrap(), read_ulstring(path));

    let file = std::fs::File::open(path).expect("bad boy ultralight not using `file_exists`???");
    let metadata = file.metadata().unwrap();

    // Read
    let mut buffer = Box::new(vec![]);
    let mut reader = std::io::BufReader::new(file);
    reader.read_to_end(&mut buffer).unwrap();

    ulCreateBuffer(
        buffer.as_mut_ptr() as _,
        metadata.len() as usize,
        Box::into_raw(buffer) as *mut _,
        Some(close_file),
    )
}

unsafe extern "C" fn close_file(user_data: *mut c_void, _data: *mut c_void) {
    // Drop boxed buffer
    drop(Box::from_raw(user_data as *mut _));
}
