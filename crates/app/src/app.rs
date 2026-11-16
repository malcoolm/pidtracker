//! Kořenová komponenta: stav, start služeb, přepínání obrazovek.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::Toast;
use crate::config::{APP_VERSION, BUILD_ID};
use crate::season::{HalloweenDeco, apply_season};
use pid_core::season::Season;
use crate::screens::{AlertsScreen, Alight, Board, Edit, History, Home, Onboarding, SearchScreen, Settings, Stats};
use crate::state::{AppState, Screen};
use crate::topbar::TopBar;
use crate::{storage, sync, timetable_loader, update};

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();
    provide_context(state);

    spawn_local(async move {
        match storage::all_rides().await {
            Ok(rides) => state.rides.set(rides),
            Err(e) => state.show_toast(format!("Lokální data nejdou načíst: {}", e.0)),
        }
        sync::install_triggers(state);
    });
    timetable_loader::install(state);
    update::install(state);
    crate::alerts::install(state);
    crate::reauth::install(state);

    // Sezónní vzhled podle data; přepočet při návratu do popředí (např. 5. 11. ráno).
    let season = RwSignal::new(apply_season());
    crate::interop::on_visible(move || {
        season.set(apply_season());
        state.foreground.update(|n| *n += 1);
    });

    let needs_setup = move || state.me.get().is_none();

    view! {
        <Show when=move || !needs_setup() fallback=|| view! { <main class="app"><Onboarding /></main> }>
            <TopBar />
            <main class="app">
                {move || match state.screen.get() {
                    Screen::Home => view! { <Home /> }.into_any(),
                    Screen::Search => view! { <SearchScreen /> }.into_any(),
                    Screen::Board { group } => view! { <Board group=group /> }.into_any(),
                    Screen::Alight => view! { <Alight /> }.into_any(),
                    Screen::History => view! { <History /> }.into_any(),
                    Screen::Edit(id) => view! { <Edit id=id /> }.into_any(),
                    Screen::Stats => view! { <Stats /> }.into_any(),
                    Screen::Settings => view! { <Settings /> }.into_any(),
                    Screen::Alerts { open } => view! { <AlertsScreen open=open /> }.into_any(),
                }}
                <footer class="version">{format!("v{APP_VERSION} ({BUILD_ID})")}</footer>
            </main>
        </Show>
        <Toast />
        <Show when=move || season.get() == Some(Season::Halloween)>
            <HalloweenDeco />
        </Show>
    }
}
