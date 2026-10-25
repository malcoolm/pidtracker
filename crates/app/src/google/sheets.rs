//! Tenký klient Google Sheets API v4.

use gloo_net::http::{Request, Response};
use pid_core::sheet::column_letter;
use serde::Deserialize;
use serde_json::{Value, json};

const BASE: &str = "https://sheets.googleapis.com/v4/spreadsheets";

#[derive(Clone, Debug, PartialEq)]
pub enum SheetsError {
    /// Token je neplatný nebo vypršel.
    Unauthorized,
    /// K tabulce není přístup (neexistuje nebo není nasdílená).
    Forbidden,
    Network(String),
    Other(String),
}

impl SheetsError {
    pub fn message(&self) -> String {
        match self {
            SheetsError::Unauthorized => "Přihlášení vypršelo.".into(),
            SheetsError::Forbidden => "K tabulce nemáte přístup. Zkontrolujte odkaz a sdílení.".into(),
            SheetsError::Network(m) => format!("Síťová chyba: {m}"),
            SheetsError::Other(m) => m.clone(),
        }
    }
}

pub struct SheetsClient {
    token: String,
    sheet_id: String,
}

fn enc(s: &str) -> String {
    String::from(js_sys::encode_uri_component(s))
}

impl SheetsClient {
    pub fn new(token: &str, sheet_id: &str) -> Self {
        SheetsClient { token: token.to_string(), sheet_id: sheet_id.to_string() }
    }

    fn url(&self, suffix: &str) -> String {
        format!("{BASE}/{}{suffix}", self.sheet_id)
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    async fn check(resp: Result<Response, gloo_net::Error>) -> Result<Value, SheetsError> {
        let resp = resp.map_err(|e| SheetsError::Network(e.to_string()))?;
        match resp.status() {
            200..=299 => resp.json().await.map_err(|e| SheetsError::Other(e.to_string())),
            401 => Err(SheetsError::Unauthorized),
            403 | 404 => Err(SheetsError::Forbidden),
            s => Err(SheetsError::Other(format!(
                "Google Sheets HTTP {s}: {}",
                resp.text().await.unwrap_or_default()
            ))),
        }
    }

    async fn get_json(&self, url: &str) -> Result<Value, SheetsError> {
        Self::check(Request::get(url).header("Authorization", &self.auth()).send().await).await
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, SheetsError> {
        let req = Request::post(url)
            .header("Authorization", &self.auth())
            .json(body)
            .map_err(|e| SheetsError::Other(e.to_string()))?;
        Self::check(req.send().await).await
    }

    pub async fn get_values(&self, range: &str) -> Result<Vec<Vec<String>>, SheetsError> {
        #[derive(Deserialize)]
        struct ValueRange {
            #[serde(default)]
            values: Vec<Vec<Value>>,
        }
        let v = self.get_json(&self.url(&format!("/values/{}", enc(range)))).await?;
        let vr: ValueRange = serde_json::from_value(v).map_err(|e| SheetsError::Other(e.to_string()))?;
        Ok(vr
            .values
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|c| match c {
                        Value::String(s) => s,
                        other => other.to_string(),
                    })
                    .collect()
            })
            .collect())
    }

    pub async fn append(&self, sheet: &str, rows: Vec<Vec<String>>) -> Result<(), SheetsError> {
        if rows.is_empty() {
            return Ok(());
        }
        let url = self.url(&format!(
            "/values/{}:append?valueInputOption=RAW&insertDataOption=INSERT_ROWS",
            enc(&format!("{sheet}!A1"))
        ));
        self.post_json(&url, &json!({ "values": rows })).await.map(|_| ())
    }

    pub async fn batch_update_values(&self, data: Vec<(String, Vec<Vec<String>>)>) -> Result<(), SheetsError> {
        if data.is_empty() {
            return Ok(());
        }
        let data: Vec<Value> =
            data.into_iter().map(|(range, values)| json!({ "range": range, "values": values })).collect();
        self.post_json(&self.url("/values:batchUpdate"), &json!({ "valueInputOption": "RAW", "data": data }))
            .await
            .map(|_| ())
    }

    /// Založí chybějící listy a doplní chybějící hlavičky na konec řádku 1 (nic nepřepisuje).
    pub async fn ensure_sheets(&self, sheets: &[(&str, &[&str])]) -> Result<(), SheetsError> {
        let meta = self.get_json(&self.url("?fields=sheets.properties.title")).await?;
        let existing: Vec<String> = meta["sheets"]
            .as_array()
            .map(|a| a.iter().filter_map(|s| s["properties"]["title"].as_str().map(String::from)).collect())
            .unwrap_or_default();
        let missing: Vec<Value> = sheets
            .iter()
            .filter(|(name, _)| !existing.iter().any(|e| e == name))
            .map(|(name, _)| {
                json!({ "addSheet": { "properties": { "title": name, "gridProperties": { "frozenRowCount": 1 } } } })
            })
            .collect();
        if !missing.is_empty() {
            self.post_json(&self.url(":batchUpdate"), &json!({ "requests": missing })).await?;
        }
        let mut updates = Vec::new();
        for (name, headers) in sheets {
            let current = self.get_values(&format!("{name}!1:1")).await?.into_iter().next().unwrap_or_default();
            // Řádek 1 s daty místo hlavičky (seřazeno i s ní): nic nepsat, ať nepoškodíme data.
            let has_any = current.iter().any(|c| !c.trim().is_empty());
            if has_any && !current.iter().any(|c| c.trim() == headers[0]) {
                return Err(SheetsError::Other(format!(
                    "První řádek listu {name} není hlavička (list byl asi seřazen i s ní). Vraťte hlavičku na řádek 1."
                )));
            }
            let add: Vec<String> =
                headers.iter().filter(|h| !current.iter().any(|c| c == *h)).map(|h| h.to_string()).collect();
            if !add.is_empty() {
                let col = column_letter(current.len() + 1);
                updates.push((format!("{name}!{col}1"), vec![add]));
            }
        }
        self.batch_update_values(updates).await
    }
}
