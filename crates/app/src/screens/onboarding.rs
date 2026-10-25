//! První spuštění: přihlášení a výběr Google tabulky.

use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::sheet::{HEADERS, PEOPLE_HEADERS, PEOPLE_SHEET, RIDES_SHEET, parse_sheet_id};

use crate::google::auth::{current_token, sign_in};
use crate::google::sheets::SheetsClient;
use crate::state::AppState;
use crate::sync::request_sync;
use crate::util::{keys, ls_set_str};

#[component]
pub fn Onboarding() -> impl IntoView {
    let state = expect_context::<AppState>();
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let link = RwSignal::new(String::new());

    let do_sign_in = move |_| {
        error.set(None);
        let fut = sign_in(None);
        spawn_local(async move {
            match fut.await {
                Ok(t) => state.me.set(Some(t.email)),
                Err(e) => error.set(Some(e.message())),
            }
        });
    };

    let verify = move |_| {
        let Some(id) = parse_sheet_id(&link.get()) else {
            error.set(Some("Tohle nevypadá jako odkaz na Google tabulku.".into()));
            return;
        };
        let Some(token) = current_token() else {
            error.set(Some("Přihlášení vypršelo, přihlaste se znovu.".into()));
            state.me.set(None);
            return;
        };
        error.set(None);
        busy.set(true);
        spawn_local(async move {
            let client = SheetsClient::new(&token.access_token, &id);
            match client.ensure_sheets(&[(RIDES_SHEET, &HEADERS), (PEOPLE_SHEET, &PEOPLE_HEADERS)]).await {
                Ok(()) => {
                    ls_set_str(keys::SHEET_ID, &id);
                    state.sheet_id.set(Some(id));
                    request_sync(state);
                }
                Err(e) => error.set(Some(e.message())),
            }
            busy.set(false);
        });
    };

    view! {
        <section class="onboarding">
            <h1>"PID Tracker"</h1>
            <p class="muted">"Zaznamenávání jízd pražskou MHD do vaší Google tabulky."</p>
            <Show
                when=move || state.me.get().is_some()
                fallback=move || view! {
                    <h2>"1. Přihlášení"</h2>
                    <p>"Přihlaste se Google účtem, který má přístup k tabulce s jízdami."</p>
                    <button class="primary big" on:click=do_sign_in>"Přihlásit se Googlem"</button>
                }
            >
                <h2>"2. Tabulka"</h2>
                <p>"Přihlášen(a): " <strong>{move || state.me.get()}</strong></p>
                <p>"Vložte odkaz na Google tabulku (stačí prázdná; listy si aplikace založí sama)."</p>
                <input
                    type="url"
                    placeholder="https://docs.google.com/spreadsheets/d/…"
                    prop:value=move || link.get()
                    on:input=move |ev| link.set(event_target_value(&ev))
                />
                <button class="primary big" disabled=move || busy.get() on:click=verify>
                    {move || if busy.get() { "Ověřuji…" } else { "Ověřit a pokračovat" }}
                </button>
            </Show>
            {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
        </section>
    }
}
