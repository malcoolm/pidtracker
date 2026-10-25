mod app;
mod config;

fn main() {
    console_error_panic_hook::set_once();
    register_service_worker();
    leptos::mount::mount_to_body(app::App);
}

fn register_service_worker() {
    let Some(window) = web_sys::window() else { return };
    let container = window.navigator().service_worker();
    let _ = container.register("sw.js");
}
