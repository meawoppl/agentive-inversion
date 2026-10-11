mod about_me;
mod accounts;
mod app;
mod archive_review;
mod calendar;
mod categories;
mod chat;
mod claude_auth;
mod context;
mod decisions;
mod login;
mod pipeline;
mod time;
mod todos;

fn main() {
    yew::Renderer::<app::App>::new().render();
}
