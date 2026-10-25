//! První spuštění: přihlášení Googlem a ověření přístupu k pevně nastavené tabulce.

use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::sheet::{HEADERS, PEOPLE_HEADERS, PEOPLE_SHEET, RIDES_SHEET};

use crate::config::SHEET_ID;
use crate::google::auth::sign_in;
use crate::google::sheets::{SheetsClient, SheetsError};
use crate::state::AppState;
use crate::sync::request_sync;

#[component]
pub fn Onboarding() -> impl IntoView {
    let state = expect_context::<AppState>();
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

    let do_sign_in = move |_| {
        error.set(None);
        let fut = sign_in(None);
        busy.set(true);
        spawn_local(async move {
            match fut.await {
                Ok(t) => {
                    let client = SheetsClient::new(&t.access_token, SHEET_ID);
                    match client.ensure_sheets(&[(RIDES_SHEET, &HEADERS), (PEOPLE_SHEET, &PEOPLE_HEADERS)]).await {
                        Ok(()) => {
                            state.me.set(Some(t.email));
                            request_sync(state);
                        }
                        Err(SheetsError::Forbidden) => error.set(Some(format!(
                            "Účet {} nemá přístup k tabulce s jízdami. Požádejte o nasdílení (role Editor).",
                            t.email
                        ))),
                        Err(e) => error.set(Some(e.message())),
                    }
                }
                Err(e) => error.set(Some(e.message())),
            }
            busy.set(false);
        });
    };

    view! {
        <section class="onboarding">
            <h1>"PID Tracker"</h1>
            <p class="muted">"Zaznamenávání jízd pražskou MHD do společné Google tabulky."</p>
            <p>"Přihlaste se Google účtem, se kterým je tabulka s jízdami nasdílená."</p>
            <button class="primary big" disabled=move || busy.get() on:click=do_sign_in>
                {move || if busy.get() { "Přihlašuji…" } else { "Přihlásit se Googlem" }}
            </button>
            {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
        </section>
    }
}
