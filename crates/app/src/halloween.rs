//! Halloweenský vzhled (24. 10. – 4. 11.): třída `halloween` na `<html>` a dekorace.

use leptos::prelude::*;
use pid_core::season::is_halloween;

use crate::util::now;

const THEME_COLOR: &str = "#00559e";
const HALLOWEEN_THEME_COLOR: &str = "#2a2b30";

/// Je dnes Halloween? Pro test jde vynutit parametrem `?halloween=1`.
pub fn halloween_active() -> bool {
    let forced = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .is_some_and(|q| q.contains("halloween=1"));
    forced || is_halloween(now().date_naive())
}

/// Zapne/vypne halloweenský vzhled podle dnešního data. Vrací, zda je zapnutý.
pub fn apply_season() -> bool {
    let on = halloween_active();
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return on };
    if let Some(html) = doc.document_element() {
        let classes = html.class_list();
        let _ = if on { classes.add_1("halloween") } else { classes.remove_1("halloween") };
    }
    if let Ok(Some(meta)) = doc.query_selector("meta[name=theme-color]") {
        let _ = meta.set_attribute("content", if on { HALLOWEEN_THEME_COLOR } else { THEME_COLOR });
    }
    on
}

/// Siluety: pavučina s pavoukem, čarodějnice na koštěti, kostlivci. Jen dekorace, neklikatelné.
#[component]
pub fn HalloweenDeco() -> impl IntoView {
    view! {
        <div class="hw-deco" aria-hidden="true">
            <svg class="hw-web" viewBox="0 0 90 90">
                <g fill="none" stroke="currentColor" stroke-width="1">
                    <line x1="0" y1="0" x2="90" y2="0" /><line x1="0" y1="0" x2="0" y2="90" />
                    <line x1="0" y1="0" x2="80" y2="45" /><line x1="0" y1="0" x2="45" y2="80" />
                    <line x1="0" y1="0" x2="66" y2="66" />
                    <path d="M22 0 Q16 12 0 22" /><path d="M44 0 Q34 24 0 44" />
                    <path d="M66 0 Q52 38 0 66" /><path d="M88 0 Q70 52 0 88" />
                </g>
            </svg>
            <svg class="hw-spider" viewBox="0 0 40 90">
                <line x1="20" y1="0" x2="20" y2="60" stroke="currentColor" stroke-width="1" />
                <ellipse cx="20" cy="70" rx="7" ry="8" fill="currentColor" />
                <circle cx="20" cy="60" r="4.5" fill="currentColor" />
                <g stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round">
                    <path d="M14 66 Q6 60 2 66" /><path d="M14 70 Q5 68 1 76" /><path d="M14 74 Q7 76 4 84" />
                    <path d="M15 77 Q10 82 9 89" /><path d="M26 66 Q34 60 38 66" /><path d="M26 70 Q35 68 39 76" />
                    <path d="M26 74 Q33 76 36 84" /><path d="M25 77 Q30 82 31 89" />
                </g>
            </svg>
            <svg class="hw-witch" viewBox="0 0 120 70">
                <path d="M5 52 L95 40" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
                <path d="M95 40 L118 30 L116 40 L119 48 Z" fill="currentColor" />
                <path d="M40 48 Q45 30 55 26 Q62 30 66 44 Z" fill="currentColor" />
                <path d="M52 48 L44 62 L50 62 L58 47 Z" fill="currentColor" />
                <circle cx="58" cy="22" r="6" fill="currentColor" />
                <path d="M48 18 L70 15 L66 12 L60 0 L56 13 Z" fill="currentColor" />
                <path d="M38 40 Q30 44 22 40" stroke="currentColor" stroke-width="3" fill="none" />
            </svg>
            <Skeleton class="hw-skel hw-skel-1" />
            <Skeleton class="hw-skel hw-skel-2" />
        </div>
    }
}

#[component]
fn Skeleton(class: &'static str) -> impl IntoView {
    view! {
        <svg class=class viewBox="0 0 40 80">
            <g fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
                <circle cx="20" cy="10" r="8" fill="currentColor" />
                <line x1="20" y1="18" x2="20" y2="48" />
                <path d="M12 25 Q20 22 28 25" /><path d="M12 31 Q20 28 28 31" /><path d="M13 37 Q20 34 27 37" />
                <path d="M20 22 L8 34 L4 26" /><path d="M20 22 L32 34 L37 26" />
                <path d="M14 48 Q20 52 26 48" />
                <path d="M16 50 L12 66 L8 76" /><path d="M24 50 L28 66 L33 76" />
            </g>
            <circle cx="17" cy="9" r="2" class="hw-eye" /><circle cx="23" cy="9" r="2" class="hw-eye" />
        </svg>
    }
}
