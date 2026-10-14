//! Currency exchange rates from fawazahmed0's exchange-api
//! (<https://github.com/fawazahmed0/exchange-api>), cached on disk.
//!
//! Rates are loaded from the cache at startup and refreshed in a background thread when the
//! cache is older than 12 hours, so opening the window never waits on the network and
//! conversions keep working offline with the last downloaded rates.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use anyhow::Context as _;

/// Rates relative to USD. The second URL is the Cloudflare mirror the project asks clients to
/// fall back to when jsDelivr fails.
const URLS: [&str; 2] = [
    "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/usd.min.json",
    "https://latest.currency-api.pages.dev/v1/currencies/usd.min.json",
];
/// The source updates daily; refreshing twice a day keeps rates current without needless requests.
const MAX_AGE: Duration = Duration::from_secs(12 * 60 * 60);
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// One day's rates: how many units of each currency one US dollar buys.
#[derive(Debug, Clone, PartialEq)]
pub struct Rates {
    date: String,
    per_usd: HashMap<String, f64>,
}

impl Rates {
    /// Parses the API's `{"date": "...", "usd": {"twd": 32.1, ...}}` response.
    ///
    /// # Errors
    /// Fails on invalid JSON or when the date or rate table is missing.
    pub fn parse(json: &str) -> anyhow::Result<Self> {
        let value: serde_json::Value = serde_json::from_str(json).context("invalid JSON")?;
        let date = value["date"].as_str().context("missing date")?.to_owned();
        let per_usd = value["usd"]
            .as_object()
            .context("missing usd rate table")?
            .iter()
            .filter_map(|(code, rate)| Some((code.to_ascii_lowercase(), rate.as_f64()?)))
            .filter(|(_, rate)| rate.is_finite() && *rate > 0.0)
            .collect::<HashMap<_, _>>();
        anyhow::ensure!(!per_usd.is_empty(), "empty rate table");
        Ok(Self { date, per_usd })
    }

    /// Units of `code` (any case, e.g. `TWD`) per US dollar.
    #[must_use]
    pub fn per_usd(&self, code: &str) -> Option<f64> {
        self.per_usd.get(&code.to_ascii_lowercase()).copied()
    }

    /// The day these rates are for, as `YYYY-MM-DD`.
    #[must_use]
    pub fn date(&self) -> &str {
        &self.date
    }
}

/// Rates shared between the calculator and the refresh thread; `None` until first loaded.
#[derive(Debug, Clone, Default)]
pub struct SharedRates(Arc<RwLock<Option<Rates>>>);

impl SharedRates {
    /// Rates already loaded, e.g. for tests.
    #[must_use]
    pub fn with(rates: Rates) -> Self {
        Self(Arc::new(RwLock::new(Some(rates))))
    }

    /// Date of the loaded rates, if any.
    #[must_use]
    pub fn date(&self) -> Option<String> {
        self.read(|rates| rates.map(|r| r.date.clone()))
    }

    fn set(&self, rates: Rates) {
        // A poisoned lock only means another thread panicked mid-write of a plain value.
        *self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(rates);
    }

    fn read<T>(&self, f: impl FnOnce(Option<&Rates>) -> T) -> T {
        f(self
            .0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref())
    }
}

impl fend_core::ExchangeRateFnV2 for SharedRates {
    /// fend's base currency here is USD, so "one base in currency" is the per-USD rate.
    fn relative_to_base_currency(
        &self,
        currency: &str,
        _options: &fend_core::ExchangeRateFnV2Options,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.read(|rates| match rates {
            None => Err("exchange rates not downloaded yet (needs internet once)".into()),
            Some(r) => r
                .per_usd(currency)
                .ok_or_else(|| format!("no exchange rate for {currency}").into()),
        })
    }
}

/// Loads cached rates and, if they are missing or stale, refreshes them in a background
/// thread, calling `on_update` once new rates are in place.
pub fn start(cache_file: PathBuf, on_update: impl FnOnce() + Send + 'static) -> SharedRates {
    let shared = SharedRates::default();
    match load_cache(&cache_file) {
        Ok(Some(rates)) => shared.set(rates),
        Ok(None) => {}
        Err(e) => tracing::warn!("ignoring exchange-rate cache: {e:#}"),
    }
    if is_stale(&cache_file) {
        let shared = shared.clone();
        std::thread::spawn(
            move || match fetch().and_then(|json| save(&cache_file, &json)) {
                Ok(rates) => {
                    tracing::info!("exchange rates updated to {}", rates.date());
                    shared.set(rates);
                    on_update();
                }
                Err(e) => tracing::warn!("could not update exchange rates: {e:#}"),
            },
        );
    }
    shared
}

fn load_cache(path: &Path) -> anyhow::Result<Option<Rates>> {
    match fs::read_to_string(path) {
        Ok(json) => Rates::parse(&json).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

fn is_stale(path: &Path) -> bool {
    let age = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok());
    age.is_none_or(|age| age > MAX_AGE)
}

/// Tries each URL in turn; returns the first valid response body.
fn fetch() -> anyhow::Result<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(FETCH_TIMEOUT))
        .build()
        .into();
    let mut last_error = None;
    for url in URLS {
        let result = agent
            .get(url)
            .call()
            .and_then(|mut response| response.body_mut().read_to_string())
            .with_context(|| format!("GET {url}"))
            .and_then(|body| Rates::parse(&body).map(|_| body));
        match result {
            Ok(body) => return Ok(body),
            Err(e) => {
                tracing::warn!("{e:#}");
                last_error = Some(e);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no rate URLs configured")))
}

/// Writes via a temporary file so a crash never leaves a half-written cache.
fn save(path: &Path, json: &str) -> anyhow::Result<Rates> {
    let rates = Rates::parse(json)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(rates)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"date":"2026-10-13","usd":{"usd":1,"twd":32.0,"eur":0.5,"bad":"x"}}"#;

    #[test]
    fn parses_api_response() {
        let rates = Rates::parse(SAMPLE).expect("valid sample");
        assert_eq!(rates.date(), "2026-10-13");
        assert_eq!(rates.per_usd("TWD"), Some(32.0));
        assert_eq!(rates.per_usd("bad"), None, "non-numeric rates are skipped");
    }

    #[test]
    fn rejects_malformed_response() {
        assert!(Rates::parse("not json").is_err());
        assert!(Rates::parse(r#"{"date":"2026-10-13","usd":{}}"#).is_err());
        assert!(Rates::parse(r#"{"usd":{"twd":32}}"#).is_err());
    }

    #[test]
    fn cache_round_trip_and_staleness() {
        let dir = std::env::temp_dir().join(format!("spotcalc-rates-{}", std::process::id()));
        let path = dir.join("rates.json");
        assert!(is_stale(&path), "missing cache is stale");
        assert_eq!(load_cache(&path).expect("missing is fine"), None);

        save(&path, SAMPLE).expect("save");
        assert!(!is_stale(&path), "fresh cache");
        let loaded = load_cache(&path).expect("load").expect("present");
        assert_eq!(loaded.per_usd("eur"), Some(0.5));
        fs::remove_dir_all(dir).expect("cleanup");
    }

    /// Hits the real API; run with `cargo nextest run --run-ignored all`.
    #[test]
    #[ignore = "needs network"]
    fn live_fetch_includes_twd() {
        let rates = Rates::parse(&fetch().expect("fetch")).expect("parse");
        assert!(rates.per_usd("TWD").is_some_and(|r| r > 1.0));
    }
}
