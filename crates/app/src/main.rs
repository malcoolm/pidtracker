mod app;
mod components;
mod config;
mod google;
mod interop;
mod screens;
mod state;
mod storage;
mod sync;
mod timetable_loader;
mod update;
mod util;

fn main() {
    console_error_panic_hook::set_once();
    register_service_worker();
    leptos::mount::mount_to_body(app::App);
}

fn register_service_worker() {
    let Some(window) = web_sys::window() else { return };
    let _ = window.navigator().service_worker().register("sw.js");
}
