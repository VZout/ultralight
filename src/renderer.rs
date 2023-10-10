use super::{Config, ViewConfig};
use crate::{
    sys::{
        ulBitmapGetBpp, ulBitmapGetHeight, ulBitmapGetWidth, ulBitmapRawPixels,
        ulBitmapSurfaceGetBitmap, ulBitmapSwapRedBlueChannels, ulCreateKeyEvent,
        ulCreateMouseEvent, ulCreateRenderer, ulCreateSession, ulCreateString, ulCreateView,
        ulDestroyKeyEvent, ulDestroyMouseEvent, ulDestroyRenderer, ulDestroyString, ulDestroyView,
        ulRender, ulStringGetData, ulStringGetLength, ulUpdate, ulViewFireKeyEvent,
        ulViewFireMouseEvent, ulViewFocus, ulViewGetNeedsPaint, ulViewGetSurface, ulViewLoadURL,
        ulViewLockJSContext, ulViewResize, ulViewSetAddConsoleMessageCallback,
        ulViewSetDOMReadyCallback, ulViewSetFinishLoadingCallback, ulViewUnfocus,
        ulViewUnlockJSContext, JSContextGetGlobalObject, JSContextRef,
        JSObjectMakeFunctionWithCallback, JSObjectRef, JSObjectSetProperty,
        JSStringCreateWithUTF8CString, JSStringRelease, JSValueMakeString, JSValueRef,
        ULDOMReadyCallback, ULFinishLoadingCallback, ULKeyEventType_kKeyEventType_Char,
        ULKeyEventType_kKeyEventType_KeyDown, ULKeyEventType_kKeyEventType_KeyUp, ULMessageLevel,
        ULMessageSource, ULMouseButton_kMouseButton_Left, ULMouseButton_kMouseButton_None,
        ULMouseEventType_kMouseEventType_MouseDown, ULMouseEventType_kMouseEventType_MouseMoved,
        ULMouseEventType_kMouseEventType_MouseUp, ULRenderer, ULSession, ULString, ULView,
    },
    JSContext,
};
use image::RgbaImage;
use std::{ffi::CString, os::raw::c_void, ptr::null_mut};

pub struct Renderer {
    inner: ULRenderer,
    session: ULSession,
}

impl Renderer {
    /// Create a new renderer.
    pub fn new(config: &Config) -> Self {
        let inner = unsafe { ulCreateRenderer(config.into()) };

        let text = CString::new("ulsession").unwrap();
        let text = unsafe { ulCreateString(text.as_ptr()) };
        let session = unsafe { ulCreateSession(inner, true, text) };
        unsafe { ulDestroyString(text) };

        Self { inner, session }
    }

    /// Create a View with certain size (in pixels).
    pub fn create_view(&mut self, width: u32, height: u32, config: &ViewConfig) -> View {
        let view = unsafe { ulCreateView(self.inner, width, height, config.into(), self.session) };
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
    owned: bool,
    inner: ULView,
    is_ready: Box<bool>,
}

pub extern "C" fn console_callback_wrapper(
    user_data: *mut c_void,
    _caller: ULView,
    _source: ULMessageSource,
    level: ULMessageLevel,
    message: ULString,
    _line_number: ::std::os::raw::c_uint,
    _column_number: ::std::os::raw::c_uint,
    _source_id: ULString,
) {
    let msg_length = unsafe { ulStringGetLength(message) };
    let msg_data = unsafe { ulStringGetData(message) };
    let msg_slice = unsafe { std::slice::from_raw_parts(msg_data as *const u8, msg_length) };
    let msg = String::from_utf8(msg_slice.to_vec()).unwrap();

    let safe_callback_ptr: fn(ULMessageLevel, String) = unsafe { std::mem::transmute(user_data) };
    safe_callback_ptr(level, msg);
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

    /// Set callback for the javascript console.
    /// This gets called when javascript calls `console.log` for example.
    /// But also shows javascript warnings and errors.
    pub fn set_console_callback(&mut self, callback: fn(ULMessageLevel, String)) {
        unsafe {
            let callback_ptr = callback as *mut c_void;
            ulViewSetAddConsoleMessageCallback(
                self.inner,
                Some(console_callback_wrapper),
                callback_ptr,
            );
        }
    }

    /// Set callback for when the page finishes loading a URL into a frame.
    pub fn set_dom_ready_callback(&mut self, callback: ULDOMReadyCallback) {
        unsafe {
            ulViewSetDOMReadyCallback(self.inner, callback, null_mut());
        }
    }

    pub fn key_event(
        &mut self,
        virtual_key_code: i32,
        native_key_code: i32,
        modifiers: u32,
        pressed: bool,
    ) {
        unsafe {
            let event = ulCreateKeyEvent(
                if pressed {
                    ULKeyEventType_kKeyEventType_KeyDown
                } else {
                    ULKeyEventType_kKeyEventType_KeyUp
                },
                modifiers,
                virtual_key_code,
                native_key_code,
                null_mut(),
                null_mut(),
                false,
                false,
                false,
            );

            ulViewFireKeyEvent(self.inner, event);
            ulDestroyKeyEvent(event);
        }
    }

    pub fn text_event(&mut self, text: String) {
        unsafe {
            let text = CString::new(text).unwrap();
            let text = ulCreateString(text.as_ptr());

            let event = ulCreateKeyEvent(
                ULKeyEventType_kKeyEventType_Char,
                0,
                0,
                0,
                text,
                text,
                false,
                false,
                false,
            );

            ulViewFireKeyEvent(self.inner, event);
            ulDestroyKeyEvent(event);
            ulDestroyString(text);
        }
    }

    pub fn mouse_pressed(&mut self, x: i32, y: i32, pressed: bool) {
        unsafe {
            let event = ulCreateMouseEvent(
                if pressed {
                    ULMouseEventType_kMouseEventType_MouseDown
                } else {
                    ULMouseEventType_kMouseEventType_MouseUp
                },
                x,
                y,
                ULMouseButton_kMouseButton_Left,
            );
            ulViewFireMouseEvent(self.inner, event);
            ulDestroyMouseEvent(event);
        }
    }

    pub fn mouse_moved(&mut self, x: i32, y: i32) {
        unsafe {
            let event = ulCreateMouseEvent(
                ULMouseEventType_kMouseEventType_MouseMoved,
                x,
                y,
                ULMouseButton_kMouseButton_None,
            );
            ulViewFireMouseEvent(self.inner, event);
            ulDestroyMouseEvent(event);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unsafe { ulViewResize(self.inner, width, height) };
    }

    pub fn set_focus(&mut self, bool: bool) {
        if bool {
            unsafe { ulViewFocus(self.inner) };
        } else {
            unsafe { ulViewUnfocus(self.inner) };
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

    pub fn needs_repaint(&self) -> bool {
        unsafe { ulViewGetNeedsPaint(self.inner) }
    }

    pub fn get_image_raw(&self) -> &[u8] {
        unsafe {
            let surface = ulViewGetSurface(self.inner);
            let bitmap: *mut crate::sys::C_Bitmap = ulBitmapSurfaceGetBitmap(surface);

            let width = ulBitmapGetWidth(bitmap);
            let height = ulBitmapGetHeight(bitmap);
            let pixels_ptr = ulBitmapRawPixels(bitmap);
            let bytes_per_pixel = ulBitmapGetBpp(bitmap);
            std::slice::from_raw_parts(pixels_ptr as _, (width * height * bytes_per_pixel) as usize)
        }
    }

    pub fn lock_jscontext(&self) -> JSContext<'_> {
        JSContext::new(&self)
    }

    pub fn lock_jsc(&self) {
        unsafe {
            let context = ulViewLockJSContext(self.inner);
            let global_object = JSContextGetGlobalObject(context);

            let name = CString::new("GetMessage").unwrap();
            let name = JSStringCreateWithUTF8CString(name.as_ptr());
            let func = JSObjectMakeFunctionWithCallback(context, name, Some(get_message));
            JSObjectSetProperty(context, global_object, name, func, 0, null_mut());

            JSStringRelease(name);
            ulViewUnlockJSContext(self.inner)
        };
    }
}

extern "C" fn get_message(
    ctx: JSContextRef,
    _function: JSObjectRef,
    _this_object: JSObjectRef,
    _argument_count: usize,
    _arguments: *const JSValueRef,
    _exception: *mut JSValueRef,
) -> JSValueRef {
    println!("callback");

    let string = CString::new("Hello from Rust<br/>Ultralight rocks!").unwrap();
    let string = unsafe { JSStringCreateWithUTF8CString(string.as_ptr()) };

    println!("hey");

    unsafe { JSValueMakeString(ctx, string) }
}

impl Drop for View {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                ulViewSetFinishLoadingCallback(self.inner, None, null_mut());
                ulDestroyView(self.inner);
            }
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
            owned: true,
        }
    }
}

impl From<&ULView> for View {
    fn from(value: &ULView) -> Self {
        Self {
            inner: value.clone(),
            is_ready: Box::new(false),
            owned: false,
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
