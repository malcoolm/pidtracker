//! Nastavení: účet, tabulka, přezdívka, verze a aktualizace.

use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::timetable::query::DepartureSource;

use crate::config::{APP_VERSION, BUILD_ID, BUILT_AT};
use crate::google::auth::{current_token, sign_in, sign_out};
use crate::state::{AppState, TtState};
use crate::sync::{request_sync, save_nickname};
use crate::update::{UpdateCheck, apply_if_safe, check_now};

#[component]
pub fn Settings() -> impl IntoView {
    let state = expect_context::<AppState>();
    let me = move || state.me.get().unwrap_or_default();
    let nickname = RwSignal::new(state.nickname(&me().to_lowercase()));
    let update_msg = RwSignal::new(None::<String>);
    let signed_in = RwSignal::new(current_token().is_some());

    let save_nick = move |_| {
        let nick = nickname.get_untracked().trim().to_string();
        spawn_local(async move {
            match save_nickname(state, nick).await {
                Ok(()) => state.show_toast("Přezdívka uložena"),
                Err(e) => state.show_toast(format!("Přezdívku nejde uložit: {}", e.message())),
            }
        });
    };
    let do_sign_in = move |_| {
        let fut = sign_in(state.me.get_untracked().as_deref());
        spawn_local(async move {
            match fut.await {
                Ok(_) => {
                    signed_in.set(true);
                    request_sync(state);
                }
                Err(e) => state.show_toast(e.message()),
            }
        });
    };
    let do_sign_out = move |_| {
        sign_out();
        state.me.set(None);
    };
    let check_updates = move |_| {
        update_msg.set(Some("Kontroluji…".into()));
        spawn_local(async move {
            let msg = match check_now().await {
                UpdateCheck::UpToDate => "Máte nejnovější verzi.".to_string(),
                UpdateCheck::Available(v) => format!("Je dostupná verze {v}, načte se na úvodní obrazovce."),
                UpdateCheck::Offline => "Nelze ověřit (offline?).".to_string(),
            };
            update_msg.set(Some(msg));
            apply_if_safe(&state);
        });
    };
    let sheet_url =
        move || state.sheet_id.get().map(|id| format!("https://docs.google.com/spreadsheets/d/{id}/edit"));

    view! {
        <section class="settings">
            <h2>"Nastavení"</h2>

            <h3>"Účet"</h3>
            <p>{me}</p>
            <div class="row">
                <Show when=move || !signed_in.get()>
                    <button class="secondary" on:click=do_sign_in>"Přihlásit pro synchronizaci"</button>
                </Show>
                <button class="link danger" on:click=do_sign_out>"Odhlásit"</button>
            </div>

            <h3>"Přezdívka"</h3>
            <div class="row">
                <input type="text" prop:value=move || nickname.get() on:input=move |ev| nickname.set(event_target_value(&ev)) />
                <button class="secondary" on:click=save_nick>"Uložit"</button>
            </div>

            <h3>"Tabulka"</h3>
            <div class="row">
                {move || sheet_url().map(|u| view! { <a href=u target="_blank" rel="noopener">"Otevřít v Google Sheets"</a> })}
            </div>

            <h3>"O aplikaci"</h3>
            <dl class="about">
                <dt>"Verze"</dt><dd>{format!("v{APP_VERSION}")}</dd>
                <dt>"Build"</dt><dd>{BUILD_ID}</dd>
                <dt>"Sestaveno"</dt><dd>{BUILT_AT}</dd>
                <dt>"Jízdní řád"</dt>
                <dd>{move || match state.tt.get() {
                    TtState::Ready(ix) => format!("{} (platnost {} – {})", ix.version(), ix.tt.feed_start.format("%-d. %-m."), ix.tt.feed_end.format("%-d. %-m. %Y")),
                    TtState::Loading => "načítám…".into(),
                    TtState::Missing(_) => "není k dispozici".into(),
                }}</dd>
            </dl>
            <button class="secondary" on:click=check_updates>"Zkontrolovat aktualizace"</button>
            {move || update_msg.get().map(|m| view! { <p class="muted">{m}</p> })}
        </section>
    }
}
