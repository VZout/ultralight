use std::{ffi::CString, ptr::null_mut};

use ultralight::{sys::*, Config, Renderer, ViewConfig};

/// Extremely simple example loading and rendering page.html
/// Then writing it to disc as a PNG file.
pub fn main() {
    ultralight::init("./examples/assets/".to_owned(), None);

    let mut config = Config::default();
    config.set_resource_path_prefix("../resources/".to_owned());

    let mut renderer = Renderer::new(&config);
    let mut view: ultralight::View = renderer.create_view(800, 800, &ViewConfig::default());

    view.load_url("file:///javascript.html".to_owned());
    view.set_dom_ready_callback(|view| {
        unsafe {
            let context = &view.lock_jscontext();
            let global_object = &context.get_global_object();

            let name = CString::new("GetMessage").unwrap();
            let name = JSStringCreateWithUTF8CString(name.as_ptr());
            let func = JSObjectMakeFunctionWithCallback(context.into(), name, Some(get_message));
            JSObjectSetProperty(
                context.into(),
                global_object.into(),
                name,
                func,
                0,
                null_mut(),
            );
            JSStringRelease(name);
        };
    });

    // Wait for page to be loaded.
    while !view.is_ready() {
        renderer.update();
    }

    view.mouse_pressed(400, 400, true);
    view.mouse_pressed(400, 400, false);

    for _ in 0..30 {
        renderer.update();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    renderer.render();

    view.get_image().save("test.png").unwrap();
}

extern "C" fn get_message(
    ctx: JSContextRef,
    _function: JSObjectRef,
    _this_object: JSObjectRef,
    _argument_count: usize,
    _arguments: *const JSValueRef,
    _exception: *mut JSValueRef,
) -> JSValueRef {
    let string = CString::new("Hello from Rust<br/>Ultralight rocks!").unwrap();
    let string = unsafe { JSStringCreateWithUTF8CString(string.as_ptr()) };

    unsafe { JSValueMakeString(ctx, string) }
}
