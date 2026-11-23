use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const MAX_BYTES: usize = 64 * 1024;
const GOOGLE_DEFAULT_ICON_SHA256: [u8; 32] = [
    0x59, 0xbf, 0xe9, 0xbc, 0x38, 0x5a, 0xd6, 0x9f, 0x50, 0x79, 0x3c, 0xe4, 0xa5, 0x33, 0x97, 0x31,
    0x6d, 0x7a, 0x87, 0x5a, 0x71, 0x48, 0xa6, 0x3c, 0x16, 0xdf, 0x9b, 0x67, 0x4c, 0x6c, 0xda, 0x64,
];

static CLIENT: OnceLock<Client> = OnceLock::new();
static CACHE: OnceLock<Mutex<HashMap<String, Option<Vec<u8>>>>> = OnceLock::new();

/// Returns cached or freshly fetched favicon bytes for a validated host.
pub fn fetch(host: &str) -> Result<Vec<u8>, String> {
    let host = validate(host)?;
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(cached) = cache
        .lock()
        .map_err(|_| "Favicon cache unavailable".to_owned())?
        .get(&host)
    {
        return cached
            .clone()
            .ok_or_else(|| "Favicon unavailable".to_owned());
    }

    let bytes = download(&format!(
        "https://www.google.com/s2/favicons?domain={host}&sz=64"
    ))
    .ok()
    .filter(|bytes| !is_default_google_icon(bytes))
    .or_else(|| download(&format!("https://icons.duckduckgo.com/ip3/{host}.ico")).ok())
    .ok_or_else(|| "Favicon unavailable".to_owned())?;

    if let Ok(mut cache) = cache.lock() {
        cache.insert(host, Some(bytes.clone()));
    }
    Ok(bytes)
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let response = client()
        .get(url)
        .send()
        .map_err(|error| format!("Favicon request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Favicon request failed: {error}"))?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    if !content_type.starts_with("image/") {
        return Err("Favicon response is not an image".to_owned());
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("Favicon read failed: {error}"))?
        .to_vec();
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("Favicon has an invalid size".to_owned());
    }
    Ok(bytes)
}

fn is_default_google_icon(bytes: &[u8]) -> bool {
    Sha256::digest(bytes).as_slice() == GOOGLE_DEFAULT_ICON_SHA256.as_slice()
}

fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(3))
            .user_agent(concat!("kurama-host/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("favicon client")
    })
}

fn validate(host: &str) -> Result<String, String> {
    let host = host.trim().to_lowercase();
    let valid = !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains("..")
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if valid {
        Ok(host)
    } else {
        Err("Invalid favicon host".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_accepts_hosts_and_rejects_urls() {
        assert_eq!(validate("WWW.Example.com").unwrap(), "www.example.com");
        assert!(validate("https://example.com").is_err());
        assert!(validate("user@example.com").is_err());
        assert!(validate("example.com:443").is_err());
    }
}
