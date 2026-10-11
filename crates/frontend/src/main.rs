mod app;
mod connector;
mod login;

fn main() {
    yew::Renderer::<app::App>::new().render();
}
