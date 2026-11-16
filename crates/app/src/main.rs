mod alerts;
mod app;
mod components;
mod config;
mod google;
mod season;
mod interop;
mod realtime;
mod reauth;
mod screens;
mod state;
mod stop_input;
mod topbar;
mod storage;
mod sync;
mod timetable_loader;
mod update;
mod util;

fn main() {
    console_error_panic_hook::set_once();
    register_service_worker();
    interop::persist_storage();
    leptos::mount::mount_to_body(app::App);
}

fn register_service_worker() {
    let Some(window) = web_sys::window() else { return };
    let _ = window.navigator().service_worker().register("sw.js");
}
