use ultralight::{Config, Renderer, ViewConfig};

pub fn main() {
    ultralight::init("./examples/assets/".to_owned());

    let mut config = Config::default();
    config.set_resource_path_prefix("../resources/".to_owned());

    let mut renderer = Renderer::new(&config);
    let mut view = renderer.create_view(800, 800, &ViewConfig::default());

    view.load_url("file:///page.html".to_owned());

    // Wait for page to be loaded.
    while !view.is_ready() {
        renderer.update();
    }

    renderer.render();

    let output = view.get_image();
    output.save("test.png").unwrap();
}
