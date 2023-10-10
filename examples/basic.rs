use ultralight::{Config, Renderer, ViewConfig};

/// Extremely simple example loading and rendering page.html
/// Then writing it to disc as a PNG file.
pub fn main() {
    ultralight::init("./examples/assets/".to_owned(), None);

    let mut config = Config::default();
    config.set_resource_path_prefix("../resources/".to_owned());

    let mut renderer = Renderer::new(&config);
    let mut view: ultralight::View = renderer.create_view(800, 800, &ViewConfig::default());

    view.load_url("file:///page.html".to_owned());

    // Wait for page to be loaded.
    while !view.is_ready() {
        renderer.update();
    }

    renderer.render();

    view.get_image().save("test.png").unwrap();
}
