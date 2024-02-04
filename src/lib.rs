pub mod javascript;
pub mod platform;
pub mod renderer;
pub mod sys;

pub use javascript::*;
pub use platform::*;
pub use renderer::*;
use sys::{ulConfigSetAnimationTimerDelay, ulConfigSetCachePath, ulViewConfigSetIsTransparent};

use crate::sys::{
    ulConfigSetResourcePathPrefix, ulCreateConfig, ulCreateString, ulCreateViewConfig,
    ulDestroyConfig, ulDestroyString, ulDestroyViewConfig, ulViewConfigSetInitialDeviceScale,
    ulViewConfigSetIsAccelerated, ULConfig, ULViewConfig,
};
use std::ffi::CString;

pub struct Config {
    inner: ULConfig,
}

impl Config {
    pub fn set_resource_path_prefix(&mut self, path: String) {
        let path = CString::new(path).unwrap();
        unsafe {
            let path = ulCreateString(path.as_ptr());
            ulConfigSetResourcePathPrefix(self.inner, path);
            ulConfigSetAnimationTimerDelay(self.inner, 0.0);
            ulDestroyString(path);
        }
    }

    pub fn set_cache_path(&mut self, path: String) {
        let path = CString::new(path).unwrap();
        unsafe {
            let path = ulCreateString(path.as_ptr());
            ulConfigSetCachePath(self.inner, path);
            ulDestroyString(path);
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            inner: unsafe { ulCreateConfig() },
        }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        unsafe {
            ulDestroyConfig(self.inner);
        }
    }
}

impl From<&Config> for ULConfig {
    fn from(value: &Config) -> Self {
        value.inner
    }
}

pub struct ViewConfig {
    inner: ULViewConfig,
}

impl Default for ViewConfig {
    fn default() -> Self {
        let inner = unsafe { ulCreateViewConfig() };
        unsafe {
            ulViewConfigSetInitialDeviceScale(inner, 1.0);
            ulViewConfigSetIsAccelerated(inner, false);
            ulViewConfigSetIsTransparent(inner, true);
        }

        Self { inner }
    }
}

impl Drop for ViewConfig {
    fn drop(&mut self) {
        unsafe {
            ulDestroyViewConfig(self.inner);
        }
    }
}

impl From<&ViewConfig> for ULViewConfig {
    fn from(value: &ViewConfig) -> Self {
        value.inner
    }
}
