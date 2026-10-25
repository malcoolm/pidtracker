//! Kořenová komponenta aplikace.

use leptos::prelude::*;

use crate::config::{APP_VERSION, BUILD_ID};

#[component]
pub fn App() -> impl IntoView {
    view! {
        <main class="app">
            <h1>"PID Tracker"</h1>
        </main>
        <footer class="version">{format!("v{APP_VERSION} ({BUILD_ID})")}</footer>
    }
}
