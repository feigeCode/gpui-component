mod showcase;

fn main() {
    let component = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "overview".to_string());

    let app = gpui_platform::application();
    showcase::run(app, component);
}
