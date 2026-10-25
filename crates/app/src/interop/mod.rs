//! Most do API prohlížeče, která nemají pohodlné Rust bindingy.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export function getPosition(timeoutMs) {
  return new Promise((resolve, reject) => {
    if (!('geolocation' in navigator)) { reject(2); return; }
    navigator.geolocation.getCurrentPosition(
      (p) => resolve({ lat: p.coords.latitude, lon: p.coords.longitude, accuracy: p.coords.accuracy }),
      (e) => reject(e.code),
      { enableHighAccuracy: true, timeout: timeoutMs, maximumAge: 30000 },
    );
  });
}

// Promise se vytváří synchronně, aby requestAccessToken běžel v rámci uživatelského gesta.
export function gisRequestToken(clientId, scope, hint) {
  return new Promise((resolve, reject) => {
    const oauth2 = window.google && window.google.accounts && window.google.accounts.oauth2;
    if (!oauth2) { reject('Google přihlášení se ještě nenačetlo, zkuste to za chvíli.'); return; }
    const client = oauth2.initTokenClient({
      client_id: clientId,
      scope,
      callback: (r) => r.error ? reject(r.error_description || r.error) : resolve(r),
      error_callback: (e) => reject((e && (e.message || e.type)) || 'přihlášení se nezdařilo'),
    });
    const opts = {};
    if (hint) opts.hint = hint;
    client.requestAccessToken(opts);
  });
}

export function gisRevoke(token) {
  try { window.google.accounts.oauth2.revoke(token, () => {}); } catch (_) {}
}

export async function purgeDataCache(keepFile) {
  if (!('caches' in self)) return;
  const cache = await caches.open('pid-data');
  for (const req of await cache.keys()) {
    const path = new URL(req.url).pathname;
    if (path.endsWith('.pidt') && !path.endsWith('/' + keepFile)) await cache.delete(req);
  }
}

export async function swUpdate() {
  if (!('serviceWorker' in navigator)) return;
  const reg = await navigator.serviceWorker.getRegistration();
  if (reg) await reg.update();
}

export function onVisible(cb) {
  document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible') cb(); });
}

export function onOnline(cb) {
  window.addEventListener('online', () => cb());
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = getPosition, catch)]
    async fn get_position_js(timeout_ms: u32) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = gisRequestToken)]
    pub fn gis_request_token(client_id: &str, scope: &str, hint: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = gisRevoke)]
    pub fn gis_revoke(token: &str);

    #[wasm_bindgen(js_name = purgeDataCache, catch)]
    async fn purge_data_cache_js(keep_file: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = swUpdate, catch)]
    async fn sw_update_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = onVisible)]
    fn on_visible_js(cb: &Closure<dyn FnMut()>);

    #[wasm_bindgen(js_name = onOnline)]
    fn on_online_js(cb: &Closure<dyn FnMut()>);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fix {
    pub lat: f64,
    pub lon: f64,
    pub accuracy_m: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeoError {
    Denied,
    Unavailable,
    Timeout,
}

impl GeoError {
    pub fn message(&self) -> &'static str {
        match self {
            GeoError::Denied => "Přístup k poloze je zakázaný. Povolte ho v Nastavení → Soukromí → Polohové služby, nebo zastávku vyhledejte.",
            GeoError::Unavailable => "Polohu se nepodařilo zjistit. Zastávku vyhledejte podle názvu.",
            GeoError::Timeout => "Zjištění polohy trvá příliš dlouho. Zastávku vyhledejte podle názvu.",
        }
    }
}

pub async fn current_position() -> Result<Fix, GeoError> {
    match get_position_js(10_000).await {
        Ok(v) => {
            let num = |k: &str| js_sys::Reflect::get(&v, &k.into()).ok().and_then(|x| x.as_f64()).unwrap_or(0.0);
            Ok(Fix { lat: num("lat"), lon: num("lon"), accuracy_m: num("accuracy") })
        }
        Err(code) => Err(match code.as_f64() {
            Some(1.0) => GeoError::Denied,
            Some(3.0) => GeoError::Timeout,
            _ => GeoError::Unavailable,
        }),
    }
}

pub async fn purge_data_cache_except(file: &str) {
    let _ = purge_data_cache_js(file).await;
}

pub async fn sw_update() {
    let _ = sw_update_js().await;
}

/// Zavolá `f` při návratu aplikace do popředí.
pub fn on_visible(f: impl FnMut() + 'static) {
    let cb = Closure::<dyn FnMut()>::new(f);
    on_visible_js(&cb);
    cb.forget();
}

/// Zavolá `f`, když se zařízení připojí k síti.
pub fn on_online(f: impl FnMut() + 'static) {
    let cb = Closure::<dyn FnMut()>::new(f);
    on_online_js(&cb);
    cb.forget();
}
