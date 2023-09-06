use std::{ffi::CString, mem::ManuallyDrop};

use crate::sys::{ulCreateString, ulDestroyString, C_String, ULString};

pub struct UiString {
    pub(crate) inner: *mut C_String,
}

impl From<String> for UiString {
    fn from(value: String) -> Self {
        let str = ManuallyDrop::new(CString::new(value).unwrap());
        let inner = unsafe { ulCreateString(str.as_ptr()) };
        Self { inner }
    }
}

impl From<&str> for UiString {
    fn from(value: &str) -> Self {
        let str = ManuallyDrop::new(CString::new(value).unwrap());
        let inner = unsafe { ulCreateString(str.as_ptr()) };
        Self { inner }
    }
}

impl From<&UiString> for ULString {
    fn from(value: &UiString) -> Self {
        value.inner
    }
}

impl Drop for UiString {
    fn drop(&mut self) {
        unsafe {
            ulDestroyString(self.inner);
        }
    }
}
