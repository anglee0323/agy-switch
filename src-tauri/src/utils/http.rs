use once_cell::sync::Lazy;
use rquest::Client;
use rquest_util::Emulation;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

// Retain pooling for bursts, but do not freeze system proxies at first startup.
static CLIENTS: Lazy<Mutex<HashMap<u8, (Instant, Client)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
const PROXY_POLL_INTERVAL: Duration = Duration::from_secs(2);

pub fn invalidate_clients() {
    if let Ok(mut clients) = CLIENTS.lock() {
        clients.clear();
    }
}

fn get(kind: u8) -> Result<Client, String> {
    let mut clients = CLIENTS
        .lock()
        .map_err(|_| "HTTP client cache unavailable.")?;
    if let Some((created, client)) = clients.get(&kind) {
        if created.elapsed() < PROXY_POLL_INTERVAL {
            return Ok(client.clone());
        }
    }
    let mut builder =
        Client::builder().timeout(Duration::from_secs(if kind == 2 { 60 } else { 15 }));
    if kind == 0 {
        builder = builder.emulation(Emulation::Chrome123);
    }
    let client = super::proxy::configure_rquest(builder, &super::proxy::resolve()?)?
        .build()
        .map_err(|_| "Cannot initialize the HTTP client. Check proxy settings.")?;
    clients.insert(kind, (Instant::now(), client.clone()));
    Ok(client)
}

pub fn get_client() -> Result<Client, String> {
    get(0)
}
pub fn get_standard_client() -> Result<Client, String> {
    get(1)
}
pub fn get_long_standard_client() -> Result<Client, String> {
    get(2)
}
