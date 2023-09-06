use super::{Config, ViewConfig};
use crate::sys::{
    ulBitmapGetBpp, ulBitmapGetHeight, ulBitmapGetWidth, ulBitmapRawPixels,
    ulBitmapSurfaceGetBitmap, ulBitmapSwapRedBlueChannels, ulCreateRenderer, ulCreateString,
    ulCreateView, ulDestroyRenderer, ulDestroyString, ulDestroyView, ulRender, ulUpdate,
    ulViewGetSurface, ulViewLoadURL, ulViewSetFinishLoadingCallback, ULFinishLoadingCallback,
    ULRenderer, ULString, ULView,
};
use image::RgbaImage;
use std::{ffi::CString, os::raw::c_void, ptr::null_mut};

pub struct Renderer {
    inner: ULRenderer,
}

impl Renderer {
    /// Create a new renderer.
    pub fn new(config: &Config) -> Self {
        let inner = unsafe { ulCreateRenderer(config.into()) };
        Self { inner }
    }

    /// Create a View with certain size (in pixels).
    pub fn create_view(&mut self, width: u32, height: u32, config: &ViewConfig) -> View {
        let view = unsafe { ulCreateView(self.inner, width, height, config.into(), null_mut()) };
        let mut view = View::from(view);
        view.set_finish_loading_callback(Some(on_finish_loading));
        view
    }

    /// Render all active `Views`.
    pub fn render(&mut self) {
        unsafe {
            ulRender(self.inner);
        }
    }

    /// Update timers and dispatch internal callbacks (JavaScript and network).
    pub fn update(&mut self) {
        unsafe {
            ulUpdate(self.inner);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            ulDestroyRenderer(self.inner);
        }
    }
}

impl From<Renderer> for ULRenderer {
    fn from(value: Renderer) -> Self {
        value.inner
    }
}

pub struct View {
    inner: ULView,
    is_ready: Box<bool>,
}

impl View {
    /// Set callback for when the page finishes loading a URL into a frame.
    pub fn set_finish_loading_callback(&mut self, callback: ULFinishLoadingCallback) {
        unsafe {
            ulViewSetFinishLoadingCallback(
                self.inner,
                callback,
                self.is_ready.as_mut() as *mut _ as _,
            );
        }
    }

    /// Load a URL into main frame.
    pub fn load_url(&mut self, string: String) {
        unsafe {
            let url_string = CString::new(string).unwrap();
            let url_string = ulCreateString(url_string.as_ptr());
            ulViewLoadURL(self.inner, url_string);
            ulDestroyString(url_string);
        }
    }

    /// Returns whether the main frame is loaded.
    pub fn is_ready(&self) -> bool {
        *self.is_ready
    }

    /// Get the surface of the `View` as a `RgbaImage`.
    pub fn get_image(&self) -> RgbaImage {
        unsafe {
            let surface = ulViewGetSurface(self.inner);
            let bitmap: *mut crate::sys::C_Bitmap = ulBitmapSurfaceGetBitmap(surface);

            let width = ulBitmapGetWidth(bitmap);
            let height = ulBitmapGetHeight(bitmap);
            ulBitmapSwapRedBlueChannels(bitmap);
            let pixels_ptr = ulBitmapRawPixels(bitmap);
            let bytes_per_pixel = ulBitmapGetBpp(bitmap);
            let pixels: &[u8] = std::slice::from_raw_parts(
                pixels_ptr as _,
                (width * height * bytes_per_pixel) as usize,
            );

            RgbaImage::from_vec(width, height, pixels.to_vec()).unwrap()
        }
    }
}

impl Drop for View {
    fn drop(&mut self) {
        unsafe {
            ulViewSetFinishLoadingCallback(self.inner, None, null_mut());
            ulDestroyView(self.inner);
        }
    }
}

impl From<&View> for ULView {
    fn from(value: &View) -> Self {
        value.inner
    }
}

impl From<ULView> for View {
    fn from(value: ULView) -> Self {
        Self {
            inner: value,
            is_ready: Box::new(false),
        }
    }
}

pub extern "C" fn on_finish_loading(
    user_data: *mut c_void,
    _caller: ULView,
    _frame_id: u64,
    is_main_frame: bool,
    _url: ULString,
) {
    if is_main_frame {
        let is_ready: *mut bool = user_data as _;
        unsafe { *is_ready = true };
    }
}
