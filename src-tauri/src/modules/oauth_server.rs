use crate::modules::oauth;
use std::sync::{Mutex, OnceLock};
use tauri::Url;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use tokio::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::watch;

struct OAuthFlowState {
    auth_url: String,
    redirect_uri: String,
    state: String,
    client_key: String,
    cancel_tx: watch::Sender<bool>,
    code_tx: mpsc::Sender<Result<String, String>>,
    code_rx: Option<mpsc::Receiver<Result<String, String>>>,
    expires_at: Instant,
}

static OAUTH_FLOW_STATE: OnceLock<Mutex<Option<OAuthFlowState>>> = OnceLock::new();

fn get_oauth_flow_state() -> &'static Mutex<Option<OAuthFlowState>> {
    OAUTH_FLOW_STATE.get_or_init(|| Mutex::new(None))
}

fn oauth_success_html() -> &'static str {
    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
    <html>\
    <body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
    <h1 style='color: green;'>✅ Authorization Successful!</h1>\
    <p>You can close this window and return to the application.</p>\
    <script>setTimeout(function() { window.close(); }, 2000);</script>\
    </body>\
    </html>"
}

fn oauth_fail_html() -> &'static str {
    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
    <html>\
    <body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
    <h1 style='color: red;'>❌ Authorization Failed</h1>\
    <p>Failed to obtain Authorization Code. Please return to the app and try again.</p>\
    </body>\
    </html>"
}

/// Read a complete bounded header. Preconnects, fragments and unrelated requests
/// must not consume the authorization channel or close the listener.
async fn read_callback(mut stream: TcpStream, expected_state: String) -> Option<Result<String, String>> {
    let header = tokio::time::timeout(Duration::from_secs(3), async {
        let mut data = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let count = stream.read(&mut buffer).await.ok()?;
            if count == 0 { return None; }
            data.extend_from_slice(&buffer[..count]);
            if data.len() > 8192 { return None; }
            if data.windows(4).any(|part| part == b"\r\n\r\n") { return Some(data); }
        }
    }).await.ok().flatten();
    let result = header.and_then(|bytes| {
        let request = std::str::from_utf8(&bytes).ok()?;
        let mut parts = request.lines().next()?.split_whitespace();
        if parts.next()? != "GET" { return None; }
        let target = parts.next()?;
        if !matches!(parts.next(), Some("HTTP/1.1" | "HTTP/1.0")) || parts.next().is_some() { return None; }
        let url = Url::parse(&format!("http://localhost{target}")).ok()?;
        if url.path() != "/oauth-callback" || url.fragment().is_some() { return None; }
        let values = |key: &str| url.query_pairs().filter(|(name, _)| name == key).map(|(_, value)| value.into_owned()).collect::<Vec<_>>();
        let states = values("state");
        if states.len() != 1 || states[0] != expected_state { return None; }
        let codes = values("code");
        let errors = values("error");
        if codes.len() == 1 && !codes[0].is_empty() && errors.is_empty() {
            Some(Ok(codes[0].clone()))
        } else if codes.is_empty() && errors.len() == 1 {
            Some(Err("OAuth authorization was declined. Start a new authorization flow.".into()))
        } else { None }
    });
    let response = if matches!(result, Some(Ok(_))) { oauth_success_html() } else { oauth_fail_html() };
    let _ = tokio::time::timeout(Duration::from_secs(1), stream.write_all(response.as_bytes())).await;
    result
}

async fn listen_for_callback(
    listener: TcpListener, expected_state: String,
    tx: mpsc::Sender<Result<String, String>>, mut cancel: watch::Receiver<bool>,
    expires_at: Instant, app_handle: Option<tauri::AppHandle>,
) {
    let mut connections = JoinSet::new();
    loop {
        if *cancel.borrow() { break; }
        tokio::select! {
            _ = cancel.changed() => break,
            _ = tokio::time::sleep_until(expires_at) => {
                let _ = tx.try_send(Err("OAuth authorization timed out. Start a new authorization flow.".into()));
                break;
            }
            completed = connections.join_next(), if !connections.is_empty() => {
                if let Some(Ok(Some(result))) = completed {
                    if tx.try_send(result).is_ok() {
                        if let Some(h) = &app_handle {
                            use tauri::Emitter;
                            let _ = h.emit("oauth-callback-received", ());
                        }
                    }
                    break;
                }
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) if connections.len() < 8 => {
                        connections.spawn(read_callback(stream, expected_state.clone()));
                    }
                    Ok(_) => {},
                    Err(_) => {
                        let _ = tx.try_send(Err("OAuth callback listener failed. Start a new authorization flow.".into()));
                        break;
                    }
                }
            }
        }
    }
    // Dropping JoinSet aborts idle readers; a cancelled flow releases its sockets.
}

async fn wait_for_code(
    rx: &mut mpsc::Receiver<Result<String, String>>, cancel: &mut watch::Receiver<bool>, expires_at: Instant,
) -> Result<String, String> {
    if *cancel.borrow() { return Err("OAuth cancelled".into()); }
    tokio::select! {
        biased;
        result = rx.recv() => result.unwrap_or_else(|| Err("OAuth flow channel closed unexpectedly".into())),
        _ = cancel.changed() => Err("OAuth cancelled".into()),
        _ = tokio::time::sleep_until(expires_at) => Err("OAuth authorization timed out. Start a new authorization flow.".into()),
    }
}

async fn receive_oauth_code() -> Result<(String, String, String), String> {
    let (mut rx, mut cancel, expires_at, redirect_uri, client_key, expected_state) = {
        let mut lock = get_oauth_flow_state().lock().map_err(|_| "OAuth state lock corrupted")?;
        let state = lock.as_mut().ok_or("OAuth state does not exist")?;
        let rx = state.code_rx.take().ok_or("OAuth authorization already in progress")?;
        (rx, state.cancel_tx.subscribe(), state.expires_at, state.redirect_uri.clone(), state.client_key.clone(), state.state.clone())
    };
    let result = wait_for_code(&mut rx, &mut cancel, expires_at).await;
    if let Ok(mut lock) = get_oauth_flow_state().lock() {
        // An older waiter must never clear a replacement authorization flow.
        if lock.as_ref().is_some_and(|state| state.state == expected_state) {
            if let Some(state) = lock.take() { let _ = state.cancel_tx.send(true); }
        }
    }
    Ok((result?, redirect_uri, client_key))
}

async fn ensure_oauth_flow_prepared(
    app_handle: Option<tauri::AppHandle>,
    requested_client_key: Option<String>,
) -> Result<String, String> {
    if let Ok(mut state) = get_oauth_flow_state().lock() {
        if let Some(s) = state.as_mut() {
            if let Some(requested_key) = requested_client_key.as_ref() {
                if s.client_key != requested_key.to_ascii_lowercase() {
                    let _ = s.cancel_tx.send(true);
                    *state = None;
                }
            }
        }
    }

    // Return URL if flow already exists and is still "fresh" (receiver hasn't been taken)
    if let Ok(mut state) = get_oauth_flow_state().lock() {
        if let Some(s) = state.as_mut() {
            if s.code_rx.is_some() && Instant::now() < s.expires_at {
                return Ok(s.auth_url.clone());
            } else {
                // Flow is already "in progress" (rx taken), but user requested a NEW one.
                // Force cancel the old one to allow a new attempt.
                let _ = s.cancel_tx.send(true);
                *state = None;
            }
        }
    }

    // Create loopback listeners.
    // Some browsers resolve `localhost` to IPv6 (::1). To avoid "localhost refused connection",
    // we try to listen on BOTH IPv6 and IPv4 with the same port when possible.
    let mut ipv4_listener: Option<TcpListener> = None;
    let mut ipv6_listener: Option<TcpListener> = None;

    // Prefer creating one listener on an ephemeral port first, then bind the other stack to same port.
    // If both are available -> use `http://localhost:<port>` as redirect URI.
    // If only one is available -> use an explicit IP to force correct stack.
    let port: u16;
    match TcpListener::bind("[::1]:0").await {
        Ok(l6) => {
            port = l6
                .local_addr()
                .map_err(|e| format!("failed_to_get_local_port: {}", e))?
                .port();
            ipv6_listener = Some(l6);

            match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                Ok(l4) => ipv4_listener = Some(l4),
                Err(e) => {
                    crate::modules::logger::log_warn(&format!(
                        "failed_to_bind_ipv4_callback_port_127_0_0_1:{} (will only listen on IPv6): {}",
                        port, e
                    ));
                }
            }
        }
        Err(_) => {
            let l4 = TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|e| format!("failed_to_bind_local_port: {}", e))?;
            port = l4
                .local_addr()
                .map_err(|e| format!("failed_to_get_local_port: {}", e))?
                .port();
            ipv4_listener = Some(l4);

            match TcpListener::bind(format!("[::1]:{}", port)).await {
                Ok(l6) => ipv6_listener = Some(l6),
                Err(e) => {
                    crate::modules::logger::log_warn(&format!(
                        "failed_to_bind_ipv6_callback_port_::1:{} (will only listen on IPv4): {}",
                        port, e
                    ));
                }
            }
        }
    }

    let has_ipv4 = ipv4_listener.is_some();
    let has_ipv6 = ipv6_listener.is_some();

    let redirect_uri = if has_ipv4 && has_ipv6 {
        format!("http://localhost:{}/oauth-callback", port)
    } else if has_ipv4 {
        format!("http://127.0.0.1:{}/oauth-callback", port)
    } else {
        format!("http://[::1]:{}/oauth-callback", port)
    };

    let state_str = uuid::Uuid::new_v4().to_string();
    let (auth_url, resolved_client_key) = oauth::get_auth_url_with_client(
        &redirect_uri,
        &state_str,
        requested_client_key.as_deref(),
    )?;

    // Cancellation signal (supports multiple consumers)
    let (cancel_tx, cancel_rx) = watch::channel(false);
    // Use mpsc instead of oneshot to allow multiple senders (listener OR manual input)
    let (code_tx, code_rx) = mpsc::channel::<Result<String, String>>(1);

    // Capture the expected state before accepting connections; do not race global state setup.
    let expires_at = Instant::now() + Duration::from_secs(300);
    for listener in [ipv4_listener, ipv6_listener].into_iter().flatten() {
        tokio::spawn(listen_for_callback(
            listener, state_str.clone(), code_tx.clone(), cancel_rx.clone(),
            expires_at, app_handle.clone(),
        ));
    }

    // Save state
    if let Ok(mut state) = get_oauth_flow_state().lock() {
        *state = Some(OAuthFlowState {
            auth_url: auth_url.clone(),
            redirect_uri,
            state: state_str,
            client_key: resolved_client_key,
            cancel_tx,
            code_tx,
            code_rx: Some(code_rx),
            expires_at,
        });
    }

    // Send event to frontend (for display/copying link)
    if let Some(h) = app_handle {
        use tauri::Emitter;
        let _ = h.emit("oauth-url-generated", &auth_url);
    }

    Ok(auth_url)
}

/// Pre-generate OAuth URL (does not open browser, does not block waiting for callback)
pub async fn prepare_oauth_url(
    app_handle: Option<tauri::AppHandle>,
    oauth_client_key: Option<String>,
) -> Result<String, String> {
    ensure_oauth_flow_prepared(app_handle, oauth_client_key).await
}

/// Cancel current OAuth flow
pub fn cancel_oauth_flow() {
    if let Ok(mut state) = get_oauth_flow_state().lock() {
        if let Some(s) = state.take() {
            let _ = s.cancel_tx.send(true);
            crate::modules::logger::log_info("Sent OAuth cancellation signal");
        }
    }
}

/// Start OAuth flow and wait for callback, then exchange token
pub async fn start_oauth_flow(
    app_handle: Option<tauri::AppHandle>,
    oauth_client_key: Option<String>,
) -> Result<oauth::TokenResponse, String> {
    // Ensure URL + listener are ready (this way if the user authorizes first, it won't get stuck)
    let auth_url = ensure_oauth_flow_prepared(app_handle.clone(), oauth_client_key).await?;

    if let Some(h) = app_handle {
        // Open default browser
        use tauri_plugin_opener::OpenerExt;
        h.opener()
            .open_url(&auth_url, None::<String>)
            .map_err(|e| format!("failed_to_open_browser: {}", e))?;
    }

    let (code, redirect_uri, client_key) = receive_oauth_code().await?;
    oauth::exchange_code_with_client(&code, &redirect_uri, Some(&client_key)).await
}

/// Завершить OAuth flow без открытия браузера.
/// Предполагается, что пользователь открыл ссылку вручную (или ранее была открыта),
/// а мы только ждём callback и обмениваем code на token.
pub async fn complete_oauth_flow(
    app_handle: Option<tauri::AppHandle>,
) -> Result<oauth::TokenResponse, String> {
    // Ensure URL + listeners exist
    let _ = ensure_oauth_flow_prepared(app_handle, None).await?;

    let (code, redirect_uri, client_key) = receive_oauth_code().await?;
    oauth::exchange_code_with_client(&code, &redirect_uri, Some(&client_key)).await
}

/// Manually submit an OAuth code to complete the flow.
/// This is used when the user manually copies the code/URL from the browser
/// because the localhost callback could not be reached in a remote environment.
pub async fn submit_oauth_code(
    code_input: String,
    state_input: Option<String>,
) -> Result<(), String> {
    let tx = {
        let lock = get_oauth_flow_state().lock().map_err(|e| e.to_string())?;
        if let Some(state) = lock.as_ref() {
            // Verify state if provided
            if let Some(provided_state) = state_input {
                if provided_state != state.state {
                    return Err("OAuth state mismatch (CSRF protection)".to_string());
                }
            }
            state.code_tx.clone()
        } else {
            return Err("No active OAuth flow found".to_string());
        }
    };

    // Extract code if it's a URL
    let code = if code_input.starts_with("http") {
        if let Ok(url) = Url::parse(&code_input) {
            url.query_pairs()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.to_string())
                .unwrap_or(code_input)
        } else {
            code_input
        }
    } else {
        code_input
    };

    crate::modules::logger::log_info("Received manual OAuth code submission");

    // Send to the channel
    tx.try_send(Ok(code))
        .map_err(|_| "OAuth flow already completed or is no longer waiting for a code".to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fixture(lifetime: Duration) -> (std::net::SocketAddr, mpsc::Receiver<Result<String, String>>, watch::Sender<bool>, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel(1);
        let (cancel, cancelled) = watch::channel(false);
        let task = tokio::spawn(listen_for_callback(listener, "fixture-state".into(), tx, cancelled, Instant::now() + lifetime, None));
        (address, rx, cancel, task)
    }

    async fn request(address: std::net::SocketAddr, text: &[u8]) {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(text).await.unwrap();
        let mut response = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response)).await.unwrap();
    }

    #[tokio::test]
    async fn preconnect_invalid_requests_and_fragmented_headers_do_not_consume_callback() {
        let (address, mut rx, cancel, task) = fixture(Duration::from_secs(10)).await;
        let idle = TcpStream::connect(address).await.unwrap();
        drop(TcpStream::connect(address).await.unwrap());
        for text in [
            "GET /favicon.ico HTTP/1.1\r\n\r\n",
            "GET /oauth-callback?code=fixture-code&state=wrong HTTP/1.1\r\n\r\n",
            "GET /oauth-callback?code=a&code=b&state=fixture-state HTTP/1.1\r\n\r\n",
        ] {
            request(address, text.as_bytes()).await;
            assert!(matches!(rx.try_recv(), Err(mpsc::error::TryRecvError::Empty)));
        }
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(b"GET /oauth-callback?code=fixture-").await.unwrap();
        tokio::task::yield_now().await;
        stream.write_all(b"code&state=fixture-state HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
        assert_eq!(tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap(), Some(Ok("fixture-code".into())));
        task.await.unwrap();
        assert!(TcpStream::connect(address).await.is_err());
        drop(idle);
        drop(cancel);
    }

    #[tokio::test]
    async fn authenticated_denial_is_terminal_but_never_echoes_provider_data() {
        let (address, mut rx, _cancel, task) = fixture(Duration::from_secs(10)).await;
        request(address, b"GET /oauth-callback?error=fixture-secret&state=fixture-state HTTP/1.1\r\n\r\n").await;
        let error = rx.recv().await.unwrap().unwrap_err();
        assert!(error.contains("declined"));
        assert!(!error.contains("fixture-secret"));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn oversized_headers_do_not_end_the_flow() {
        let (address, mut rx, cancel, task) = fixture(Duration::from_secs(10)).await;
        request(address, &vec![b'x'; 9000]).await;
        request(address, b"GET /oauth-callback?code=ok&state=fixture-state HTTP/1.0\r\n\r\n").await;
        assert_eq!(rx.recv().await, Some(Ok("ok".into())));
        task.await.unwrap();
        drop(cancel);
    }

    #[tokio::test]
    async fn timeout_and_cancellation_release_listeners_and_waiters() {
        for cancelled in [false, true] {
            let (address, mut rx, cancel, task) = fixture(Duration::from_millis(50)).await;
            let _idle = TcpStream::connect(address).await.unwrap();
            let mut subscription = cancel.subscribe();
            if cancelled { cancel.send(true).unwrap(); }
            let error = tokio::time::timeout(Duration::from_secs(1), wait_for_code(&mut rx, &mut subscription, Instant::now() + Duration::from_millis(80))).await.unwrap().unwrap_err();
            assert!(error.contains(if cancelled { "cancelled" } else { "timed out" }));
            task.await.unwrap();
            assert!(TcpStream::connect(address).await.is_err());
        }
    }

    #[tokio::test]
    async fn an_already_queued_manual_code_does_not_block_listener_cleanup() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, mut rx) = mpsc::channel(1);
        tx.try_send(Ok("manual".into())).unwrap();
        let (_cancel, cancelled) = watch::channel(false);
        let task = tokio::spawn(listen_for_callback(listener, "fixture-state".into(), tx, cancelled, Instant::now() + Duration::from_secs(5), None));
        request(address, b"GET /oauth-callback?code=browser&state=fixture-state HTTP/1.1\r\n\r\n").await;
        tokio::time::timeout(Duration::from_secs(1), task).await.unwrap().unwrap();
        assert_eq!(rx.recv().await, Some(Ok("manual".into())));
    }
}
