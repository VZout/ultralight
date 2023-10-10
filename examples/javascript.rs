use ultralight::{Config, Renderer, View, ViewConfig};

/// Extremely simple example loading and rendering page.html
/// Then writing it to disc as a PNG file.
pub fn main() {
    ultralight::init("./examples/assets/".to_owned());

    let mut config = Config::default();
    config.set_resource_path_prefix("../resources/".to_owned());

    let mut renderer = Renderer::new(&config);
    let mut view: ultralight::View = renderer.create_view(800, 800, &ViewConfig::default());

    view.load_url("file:///javascript.html".to_owned());
    view.set_dom_ready_callback(Some(dom_ready));

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

pub extern "C" fn dom_ready(
    _user_data: *mut std::os::raw::c_void,
    caller: ultralight::sys::ULView,
    _frame_id: u64,
    _is_main_frame: bool,
    _url: ultralight::sys::ULString,
) {
    let view = View::from(&caller);
    view.lock_jsc();
}
