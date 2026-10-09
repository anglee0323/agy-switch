//! Constrained CDP transport for the optional, scoped localization feature.
//! The platform adapter must verify installation/process metadata first.
//! It never discovers arbitrary ports or returns page contents.

use serde_json::{json, Value};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use tungstenite::{client::client_with_config, protocol::WebSocketConfig, Message, WebSocket};
use url::Url;

const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_MESSAGE: usize = 1024 * 1024;
const MAX_EVENTS: usize = 64;
const MAX_PAGES: usize = 8;
const RUNTIME: &str = include_str!("../../resources/app-experiments/runtime.js");
const DICTIONARY: &str = include_str!("../../resources/app-experiments/zh-CN.json");
const REGISTRY: &str = "__ANTIGRAVITY_TOOLS_LOCALIZATION__";

#[derive(Debug, PartialEq, Eq)]
pub enum TransportError {
    InvalidEndpoint,
    UnverifiedListener,
    WrongProcess,
    WrongTarget,
    AmbiguousTargets,
    ConnectFailed,
    ProtocolError,
    RuntimeError,
    Timeout,
    Closed,
    Oversized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenerObservation {
    pub address: SocketAddr,
    pub owner_pid: u32,
}

/// Must be constructed from a COMPLETE OS socket observation for this port,
/// not inferred from connecting to it. The platform adapter supplies this proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedListener {
    address: SocketAddr,
    browser_pid: u32,
}

impl VerifiedListener {
    pub fn from_complete_observation(
        port: u16,
        browser_pid: u32,
        observations: &[ListenerObservation],
    ) -> Result<Self, TransportError> {
        if port == 0 || browser_pid == 0 || observations.is_empty() || observations.len() > 8 {
            return Err(TransportError::UnverifiedListener);
        }
        if observations.iter().any(|o| {
            o.address.port() != port || !o.address.ip().is_loopback() || o.owner_pid != browser_pid
        }) {
            return Err(TransportError::UnverifiedListener);
        }
        // No DNS, proxy or IPv4 shorthand. This release's official App uses
        // 127.0.0.1; a future IPv6 variant needs its own observed-platform test.
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        if !observations.iter().any(|o| o.address == address) {
            return Err(TransportError::UnverifiedListener);
        }
        Ok(Self {
            address,
            browser_pid,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserEndpoint {
    listener: VerifiedListener,
    path: String,
}

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl BrowserEndpoint {
    /// The port file is treated as untrusted data. Its path cannot supply a
    /// host, credentials, query, fragment, traversal or a page target endpoint.
    pub fn from_active_port_file(
        contents: &str,
        listener: VerifiedListener,
    ) -> Result<Self, TransportError> {
        if contents.len() > 256 {
            return Err(TransportError::InvalidEndpoint);
        }
        let lines: Vec<_> = contents.lines().collect();
        if lines.len() != 2 || lines[0] != listener.address.port().to_string() {
            return Err(TransportError::InvalidEndpoint);
        }
        let path = lines[1];
        let id = path
            .strip_prefix("/devtools/browser/")
            .ok_or(TransportError::InvalidEndpoint)?;
        if !safe_id(id) {
            return Err(TransportError::InvalidEndpoint);
        }
        Ok(Self {
            listener,
            path: path.to_string(),
        })
    }

    pub(crate) fn browser_pid(&self) -> u32 {
        self.listener.browser_pid
    }

    fn url(&self) -> String {
        format!("ws://{}{}", self.listener.address, self.path)
    }
}

/// Origin must come from independently verified App/server ownership evidence,
/// never from the first target returned by CDP. Do not store full page URLs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppOrigin(String);

impl AppOrigin {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
    pub fn from_verified_server_port(port: u16) -> Result<Self, TransportError> {
        if port == 0 {
            return Err(TransportError::WrongTarget);
        }
        Ok(Self(format!("https://127.0.0.1:{port}")))
    }

    fn matches(&self, raw: &str) -> bool {
        let Ok(url) = Url::parse(raw) else {
            return false;
        };
        // Reject URL-parser aliases (127.1, integer IPv4, percent-encoded host)
        // as well as credentials, remote pages, other local apps and frames.
        raw.starts_with(&(self.0.clone() + "/"))
            && url.username().is_empty()
            && url.password().is_none()
            && url.scheme() == "https"
            && url.host_str() == Some("127.0.0.1")
            && url.origin().ascii_serialization() == self.0
    }
}

#[derive(Debug, Clone)]
pub struct PageTarget {
    id: String,
    origin: AppOrigin,
}

impl PageTarget {
    pub(crate) fn origin(&self) -> &AppOrigin {
        &self.origin
    }
}

fn verified_pages(result: &Value, origin: &AppOrigin) -> Result<Vec<PageTarget>, TransportError> {
    let targets = result
        .get("targetInfos")
        .and_then(Value::as_array)
        .ok_or(TransportError::ProtocolError)?;
    if targets.len() > 128 {
        return Err(TransportError::Oversized);
    }
    let mut pages = Vec::new();
    for target in targets {
        if target.get("type").and_then(Value::as_str) != Some("page") {
            continue;
        }
        let Some(raw) = target.get("url").and_then(Value::as_str) else {
            continue;
        };
        if !origin.matches(raw) {
            continue;
        }
        let id = target
            .get("targetId")
            .and_then(Value::as_str)
            .filter(|id| safe_id(id))
            .ok_or(TransportError::WrongTarget)?;
        if pages.iter().any(|p: &PageTarget| p.id == id) {
            return Err(TransportError::AmbiguousTargets);
        }
        pages.push(PageTarget {
            id: id.to_string(),
            origin: origin.clone(),
        });
    }
    if pages.is_empty() {
        return Err(TransportError::WrongTarget);
    }
    if pages.len() > MAX_PAGES {
        return Err(TransportError::AmbiguousTargets);
    }
    Ok(pages)
}

/// Target disappearance is stronger than an origin mismatch: navigation or a
/// changed type must never be treated as proof that restoration is unnecessary.
fn target_destroyed(result: &Value, target: &PageTarget) -> Result<bool, TransportError> {
    let targets = result
        .get("targetInfos")
        .and_then(Value::as_array)
        .ok_or(TransportError::ProtocolError)?;
    if targets.len() > 128 {
        return Err(TransportError::Oversized);
    }
    let mut found = false;
    for candidate in targets {
        let id = candidate
            .get("targetId")
            .and_then(Value::as_str)
            .filter(|id| safe_id(id))
            .ok_or(TransportError::ProtocolError)?;
        found |= id == target.id;
    }
    Ok(!found)
}

fn verify_browser_pid(result: &Value, expected_pid: u32) -> Result<(), TransportError> {
    let processes = result
        .get("processInfo")
        .and_then(Value::as_array)
        .ok_or(TransportError::WrongProcess)?;
    let browsers: Vec<_> = processes
        .iter()
        .filter(|p| p.get("type").and_then(Value::as_str) == Some("browser"))
        .collect();
    if expected_pid == 0
        || browsers.len() != 1
        || browsers[0].get("id").and_then(Value::as_u64) != Some(expected_pid as u64)
    {
        return Err(TransportError::WrongProcess);
    }
    Ok(())
}

fn response_result(raw: &str, id: u64) -> Result<Option<Value>, TransportError> {
    if raw.len() > MAX_MESSAGE {
        return Err(TransportError::Oversized);
    }
    let message: Value = serde_json::from_str(raw).map_err(|_| TransportError::ProtocolError)?;
    match message.get("id").and_then(Value::as_u64) {
        None if message.get("method").and_then(Value::as_str).is_some() => return Ok(None),
        Some(response_id) if response_id == id => {}
        _ => return Err(TransportError::ProtocolError),
    }
    if message.get("error").is_some() {
        return Err(TransportError::ProtocolError);
    }
    message
        .get("result")
        .cloned()
        .map(Some)
        .ok_or(TransportError::ProtocolError)
}

#[derive(Debug, Clone, Copy)]
pub enum RuntimeAction {
    Probe,
    Apply,
    Renew,
    Dispose,
}

fn expression(
    action: RuntimeAction,
    version: &str,
    origin: &AppOrigin,
) -> Result<String, TransportError> {
    // This is the only path to Runtime.evaluate. No arbitrary script, selector
    // or dictionary is accepted from a UI caller or from a remote endpoint.
    if version.len() > 32
        || version.is_empty()
        || !version.bytes().all(|b| b.is_ascii_digit() || b == b'.')
    {
        return Err(TransportError::WrongTarget);
    }
    let expected = serde_json::to_string(&origin.0).unwrap();
    let registry = serde_json::to_string(REGISTRY).unwrap();
    let operation = match action {
        RuntimeAction::Probe | RuntimeAction::Apply => {
            let dictionary: Value =
                serde_json::from_str(DICTIONARY).map_err(|_| TransportError::RuntimeError)?;
            let config = json!({"appVersion":version,"locale":"zh-CN","dictionary":dictionary});
            if matches!(action, RuntimeAction::Probe) {
                // A probe must not dispose/recreate an active controller. For
                // first use, put the temporary registry on a private host facade.
                format!("const old=window[{registry}]; if(old) return old.brand==='antigravity-tools-scoped-localization-v1' && typeof old.probe==='function' ? old.probe() : {{status:'invalid_config'}}; const host={{document:window.document,MutationObserver:window.MutationObserver,setTimeout:window.setTimeout.bind(window),clearTimeout:window.clearTimeout.bind(window)}}; const c=({RUNTIME})(host,{config}); return c.probe();")
            } else {
                format!("const c = ({RUNTIME})(window, {config}); return c.apply();")
            }
        }
        RuntimeAction::Renew => format!(
            "const c=window[{registry}]; if(!c) return {{status:'inactive'}}; if(c.brand!=='antigravity-tools-scoped-localization-v1' || typeof c.renewLease!=='function') return {{status:'invalid_config'}}; return c.renewLease();"
        ),
        RuntimeAction::Dispose => {
            format!("const c=window[{registry}]; if(!c) return {{status:'disposed'}}; if(c.brand!=='antigravity-tools-scoped-localization-v1' || typeof c.dispose!=='function') return {{status:'invalid_config'}}; return c.dispose();")
        }
    };
    Ok(format!("(() => {{ if (location.origin !== {expected}) return {{status:'wrong_origin'}}; {operation} }})()"))
}

#[derive(Debug, PartialEq, Eq)]
pub struct RuntimeReport {
    pub status: String,
    pub active: bool,
    pub translated: u64,
    pub label_count: u64,
    pub awaiting_scope: bool,
}

fn runtime_report(result: Value) -> Result<RuntimeReport, TransportError> {
    if result.get("exceptionDetails").is_some() {
        return Err(TransportError::RuntimeError);
    }
    let value = result
        .get("result")
        .and_then(|r| r.get("value"))
        .ok_or(TransportError::RuntimeError)?;
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .ok_or(TransportError::RuntimeError)?;
    if ![
        "supported",
        "applied",
        "disposed",
        "inactive",
        "unsupported_version",
        "unsupported_dom",
        "invalid_config",
        "runtime_error",
    ]
    .contains(&status)
    {
        return Err(TransportError::RuntimeError);
    }
    // Do not return arbitrary remote exception descriptions, URLs or page data.
    Ok(RuntimeReport {
        status: status.to_string(),
        active: value
            .get("active")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        translated: value
            .get("translated")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(10000),
        label_count: value
            .get("labelCount")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(10000),
        awaiting_scope: value
            .get("awaitingScope")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

pub struct CdpTransport {
    socket: WebSocket<TcpStream>,
    next_id: u64,
    browser_pid: u32,
}

impl CdpTransport {
    pub fn connect(endpoint: BrowserEndpoint) -> Result<Self, TransportError> {
        let stream = TcpStream::connect_timeout(&endpoint.listener.address, TIMEOUT)
            .map_err(|_| TransportError::ConnectFailed)?;
        stream
            .set_read_timeout(Some(TIMEOUT))
            .map_err(|_| TransportError::ConnectFailed)?;
        stream
            .set_write_timeout(Some(TIMEOUT))
            .map_err(|_| TransportError::ConnectFailed)?;
        let config = WebSocketConfig::default()
            .read_buffer_size(8192)
            .write_buffer_size(0)
            .max_write_buffer_size(MAX_MESSAGE)
            .max_message_size(Some(MAX_MESSAGE))
            .max_frame_size(Some(MAX_MESSAGE));
        // Direct handshake on the already connected loopback stream. No proxy,
        // Origin wildcard, HTTP discovery endpoint, TLS override or redirect.
        let (socket, _) = client_with_config(endpoint.url(), stream, Some(config))
            .map_err(|_| TransportError::ConnectFailed)?;
        let mut connection = Self {
            socket,
            next_id: 0,
            browser_pid: endpoint.listener.browser_pid,
        };
        connection.verify_identity()?;
        Ok(connection)
    }

    fn call(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, TransportError> {
        if ![
            "SystemInfo.getProcessInfo",
            "Target.getTargets",
            "Target.getTargetInfo",
            "Target.attachToTarget",
            "Target.detachFromTarget",
            "Runtime.evaluate",
        ]
        .contains(&method)
        {
            return Err(TransportError::ProtocolError);
        }
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(TransportError::ProtocolError)?;
        let mut request = json!({"id":self.next_id,"method":method,"params":params});
        if let Some(session) = session {
            request["sessionId"] = Value::String(session.to_string());
        }
        let request = request.to_string();
        if request.len() > MAX_MESSAGE {
            return Err(TransportError::Oversized);
        }
        self.socket
            .send(Message::Text(request.into()))
            .map_err(|_| TransportError::Closed)?;
        let deadline = Instant::now() + TIMEOUT;
        for _ in 0..MAX_EVENTS {
            let left = deadline
                .checked_duration_since(Instant::now())
                .ok_or(TransportError::Timeout)?;
            self.socket
                .get_mut()
                .set_read_timeout(Some(left))
                .map_err(|_| TransportError::Closed)?;
            let message = self.socket.read().map_err(|e| match e {
                tungstenite::Error::Io(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    TransportError::Timeout
                }
                tungstenite::Error::Capacity(_) => TransportError::Oversized,
                _ => TransportError::Closed,
            })?;
            match message {
                Message::Text(raw) => {
                    if let Some(value) = response_result(&raw, self.next_id)? {
                        return Ok(value);
                    }
                }
                Message::Ping(_) | Message::Pong(_) => {}
                Message::Close(_) => return Err(TransportError::Closed),
                _ => return Err(TransportError::ProtocolError),
            }
        }
        Err(TransportError::ProtocolError)
    }

    fn verify_identity(&mut self) -> Result<(), TransportError> {
        let value = self.call("SystemInfo.getProcessInfo", json!({}), None)?;
        verify_browser_pid(&value, self.browser_pid)
    }

    pub fn pages(&mut self, origin: &AppOrigin) -> Result<Vec<PageTarget>, TransportError> {
        self.verify_identity()?;
        let value = self.call(
            "Target.getTargets",
            json!({"filter":[{"type":"page","exclude":false}]}),
            None,
        )?;
        verified_pages(&value, origin)
    }

    pub(crate) fn pages_for_origins(
        &mut self,
        origins: &[AppOrigin],
    ) -> Result<Vec<PageTarget>, TransportError> {
        if origins.is_empty() || origins.len() > 8 {
            return Err(TransportError::WrongTarget);
        }
        self.verify_identity()?;
        let value = self.call(
            "Target.getTargets",
            json!({"filter":[{"type":"page","exclude":false}]}),
            None,
        )?;
        let mut result = Vec::new();
        for origin in origins {
            match verified_pages(&value, origin) {
                Ok(mut pages) => result.append(&mut pages),
                Err(TransportError::WrongTarget) => {}
                Err(error) => return Err(error),
            }
        }
        if result.is_empty() {
            return Err(TransportError::WrongTarget);
        }
        if result.len() > MAX_PAGES {
            return Err(TransportError::AmbiguousTargets);
        }
        Ok(result)
    }

    pub(crate) fn page_destroyed(&mut self, target: &PageTarget) -> Result<bool, TransportError> {
        self.verify_identity()?;
        // Include every type. A still-existing target with a changed type or
        // origin is not sufficient evidence to clear pending restoration.
        let value = self.call(
            "Target.getTargets",
            json!({"filter":[{"exclude":false}]}),
            None,
        )?;
        target_destroyed(&value, target)
    }

    pub fn run(
        &mut self,
        target: &PageTarget,
        version: &str,
        action: RuntimeAction,
    ) -> Result<RuntimeReport, TransportError> {
        let script = expression(action, version, &target.origin)?;
        self.verify_identity()?;
        let info = self.call("Target.getTargetInfo", json!({"targetId":target.id}), None)?;
        let verified = verified_pages(
            &json!({"targetInfos":[info.get("targetInfo").ok_or(TransportError::WrongTarget)?]}),
            &target.origin,
        )?;
        if verified.len() != 1 || verified[0].id != target.id {
            return Err(TransportError::WrongTarget);
        }
        let attached = self.call(
            "Target.attachToTarget",
            json!({"targetId":target.id,"flatten":true}),
            None,
        )?;
        let session = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .filter(|s| safe_id(s))
            .ok_or(TransportError::ProtocolError)?
            .to_string();
        let result = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression":script, "returnByValue":true, "awaitPromise":false, "timeout":1500,
                }),
                Some(&session),
            )
            .and_then(runtime_report);
        let detached = self.call(
            "Target.detachFromTarget",
            json!({"sessionId":session}),
            None,
        );
        match (result, detached) {
            (Ok(report), Ok(_)) => Ok(report),
            (Err(e), _) => Err(e),
            (_, Err(e)) => Err(e),
        }
    }
}

impl Drop for CdpTransport {
    fn drop(&mut self) {
        let _ = self.socket.close(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation(ip: &str, port: u16, pid: u32) -> ListenerObservation {
        ListenerObservation {
            address: SocketAddr::new(ip.parse().unwrap(), port),
            owner_pid: pid,
        }
    }
    fn listener() -> VerifiedListener {
        VerifiedListener::from_complete_observation(
            45678,
            42,
            &[observation("127.0.0.1", 45678, 42)],
        )
        .unwrap()
    }
    #[test]
    fn rejects_unowned_external_and_incomplete_listener_evidence() {
        for records in [
            vec![],
            vec![observation("0.0.0.0", 45678, 42)],
            vec![observation("127.0.0.1", 45678, 43)],
            vec![observation("127.0.0.1", 1234, 42)],
            vec![
                observation("127.0.0.1", 45678, 42),
                observation("::", 45678, 42),
            ],
        ] {
            assert!(VerifiedListener::from_complete_observation(45678, 42, &records).is_err());
        }
    }
    #[test]
    fn active_port_file_cannot_redirect_or_smuggle_an_endpoint() {
        let endpoint = BrowserEndpoint::from_active_port_file(
            "45678\n/devtools/browser/ABC-def-123\n",
            listener(),
        )
        .unwrap();
        assert_eq!(endpoint.browser_pid(), 42);
        assert_eq!(
            endpoint.url(),
            "ws://127.0.0.1:45678/devtools/browser/ABC-def-123"
        );
        for raw in [
            "0\n/devtools/browser/abc",
            "45678\n//evil.test/abc",
            "45678\n/devtools/page/abc",
            "45678\n/devtools/browser/../abc",
            "45678\n/devtools/browser/a?x=1",
            "45678\n/devtools/browser/a#token",
            "45678\n/devtools/browser/a%2fb",
            "45678\n/devtools/browser/abc\nthird",
            "45678\n/devtools/browser/",
        ] {
            assert!(
                BrowserEndpoint::from_active_port_file(raw, listener()).is_err(),
                "{raw}"
            );
        }
    }
    #[test]
    fn exact_app_origin_rejects_other_local_apps_and_url_aliases() {
        let origin = AppOrigin::from_verified_server_port(45679).unwrap();
        assert!(origin.matches("https://127.0.0.1:45679/settings"));
        for raw in [
            "http://127.0.0.1:45679/",
            "https://localhost:45679/",
            "https://127.1:45679/",
            "https://2130706433:45679/",
            "https://127.0.0.1:45678/",
            "https://127.0.0.1:45679.evil.test/",
            "https://user@127.0.0.1:45679/",
            "file:///tmp/index.html",
            "data:text/html,test",
        ] {
            assert!(!origin.matches(raw), "{raw}");
        }
    }
    #[test]
    fn browser_identity_cannot_be_another_process_or_a_renderer() {
        assert!(
            verify_browser_pid(&json!({"processInfo":[{"type":"browser","id":42}]}), 42).is_ok()
        );
        for data in [
            json!({}),
            json!({"processInfo":[{"type":"renderer","id":42}]}),
            json!({"processInfo":[{"type":"browser","id":43}]}),
            json!({"processInfo":[{"type":"browser","id":42},{"type":"browser","id":42}]}),
        ] {
            assert_eq!(
                verify_browser_pid(&data, 42),
                Err(TransportError::WrongProcess)
            );
        }
    }
    #[test]
    fn target_filter_excludes_browser_tabs_frames_and_other_origins() {
        let origin = AppOrigin::from_verified_server_port(45679).unwrap();
        let targets = json!({"targetInfos":[
            {"type":"page","targetId":"app-1","url":"https://127.0.0.1:45679/"},
            {"type":"iframe","targetId":"frame-1","url":"https://127.0.0.1:45679/"},
            {"type":"page","targetId":"other-1","url":"https://example.com/"},
            {"type":"page","targetId":"other-2","url":"https://127.0.0.1:9999/"}
        ]});
        let pages = verified_pages(&targets, &origin).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id, "app-1");
    }
    #[test]
    fn only_complete_target_id_absence_proves_page_destruction() {
        let target = PageTarget {
            id: "app-1".into(),
            origin: AppOrigin::from_verified_server_port(45679).unwrap(),
        };
        assert_eq!(
            target_destroyed(&json!({"targetInfos":[]}), &target),
            Ok(true)
        );
        assert_eq!(
            target_destroyed(
                &json!({"targetInfos":[
                    {"targetId":"other-1","type":"page","url":"https://example.com/"}
                ]}),
                &target
            ),
            Ok(true)
        );
        // A target that navigated away, or changed type, still exists.
        assert_eq!(
            target_destroyed(
                &json!({"targetInfos":[
                    {"targetId":"app-1","type":"other","url":"https://example.com/"}
                ]}),
                &target
            ),
            Ok(false)
        );
        for value in [
            json!({}),
            json!({"targetInfos":[{}]}),
            json!({"targetInfos":[{"targetId":""}]}),
            json!({"targetInfos":[{"targetId":42}]}),
        ] {
            assert_eq!(
                target_destroyed(&value, &target),
                Err(TransportError::ProtocolError)
            );
        }
        assert_eq!(
            target_destroyed(
                &json!({"targetInfos":vec![json!({"targetId":"other"});129]}),
                &target
            ),
            Err(TransportError::Oversized)
        );
    }

    #[test]
    fn protocol_ids_errors_and_oversized_messages_fail_closed() {
        assert_eq!(
            response_result(r#"{"method":"Target.targetCreated","params":{}}"#, 1),
            Ok(None)
        );
        assert_eq!(
            response_result(r#"{"id":1,"result":{}}"#, 1),
            Ok(Some(json!({})))
        );
        for raw in [
            r#"{"id":2,"result":{}}"#,
            r#"{"id":1,"error":{"message":"private"}}"#,
            "bad",
        ] {
            assert_eq!(response_result(raw, 1), Err(TransportError::ProtocolError));
        }
        assert_eq!(
            response_result(&"x".repeat(MAX_MESSAGE + 1), 1),
            Err(TransportError::Oversized)
        );
    }
    #[test]
    fn evaluation_never_returns_arbitrary_remote_data_or_exceptions() {
        assert_eq!(
            runtime_report(json!({"exceptionDetails":{"text":"private"}})),
            Err(TransportError::RuntimeError)
        );
        assert_eq!(
            runtime_report(json!({"result":{"value":{"status":"secret page text"}}})),
            Err(TransportError::RuntimeError)
        );
        let report=runtime_report(json!({"result":{"value":{"status":"applied","active":true,"translated":1,"secret":"excluded"}}})).unwrap();
        assert_eq!(
            report,
            RuntimeReport {
                status: "applied".into(),
                active: true,
                translated: 1,
                label_count: 0,
                awaiting_scope: false,
            }
        );
        let origin = AppOrigin::from_verified_server_port(45679).unwrap();
        assert!(expression(RuntimeAction::Apply, "2.19.1\";evil()", &origin).is_err());
        let code = expression(RuntimeAction::Apply, "2.19.1", &origin).unwrap();
        assert!(code.contains("location.origin !== \"https://127.0.0.1:45679\""));
        assert!(code.contains("const forbidden ="));
        assert!(code.contains("const dictionary = config.dictionary.exact;"));
        assert!(code.len() < MAX_MESSAGE);
    }

    #[test]
    fn runtime_report_exposes_only_bounded_counts_and_awaiting_scope() {
        let report = runtime_report(json!({"result":{"value":{
            "status":"supported","active":true,"translated":0,"labelCount":1,"awaitingScope":true,
            "scopes":[{"id":"private remote data"}],"reason":"private remote data"
        }}}))
        .unwrap();
        assert_eq!(report.label_count, 1);
        assert!(report.awaiting_scope);
        let report = runtime_report(json!({"result":{"value":{
            "status":"applied","translated":u64::MAX,"labelCount":u64::MAX,"awaitingScope":"private remote data"
        }}})).unwrap();
        assert_eq!(report.translated, 10000);
        assert_eq!(report.label_count, 10000);
        assert!(!report.awaiting_scope);
        let report = runtime_report(json!({"result":{"value":{
            "status":"supported","labelCount":-1
        }}}))
        .unwrap();
        assert_eq!(report.label_count, 0);
    }

    #[test]
    fn read_only_probe_does_not_replace_an_active_controller() {
        let origin = AppOrigin::from_verified_server_port(45679).unwrap();
        let code = expression(RuntimeAction::Probe, "2.19.1", &origin).unwrap();
        assert!(code.contains("old.probe()"));
        assert!(code.contains("old.brand==='antigravity-tools-scoped-localization-v1' && typeof old.probe==='function'"));
        assert!(code.contains("(host,"));
        assert!(!code.contains(")(window,"));
        for (action, method, no_controller_status) in [
            (RuntimeAction::Renew, "renewLease", "inactive"),
            (RuntimeAction::Dispose, "dispose", "disposed"),
        ] {
            let code = expression(action, "2.19.1", &origin).unwrap();
            let guard = format!("if(c.brand!=='antigravity-tools-scoped-localization-v1' || typeof c.{method}!=='function') return {{status:'invalid_config'}};");
            assert!(code.contains(&guard));
            assert!(code.contains(&format!(
                "if(!c) return {{status:'{no_controller_status}'}};"
            )));
            assert!(
                code.find(&guard).unwrap() < code.find(&format!("return c.{method}();")).unwrap()
            );
        }
    }

    #[test]
    fn synthetic_unresponsive_endpoint_has_a_total_request_deadline() {
        use std::net::TcpListener;
        let server = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = server.local_addr().unwrap();
        let pid = std::process::id();
        let worker = std::thread::spawn(move || {
            let (stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let _ = ws.read().unwrap();
            std::thread::sleep(TIMEOUT + Duration::from_millis(100));
        });
        let listener = VerifiedListener::from_complete_observation(
            address.port(),
            pid,
            &[ListenerObservation {
                address,
                owner_pid: pid,
            }],
        )
        .unwrap();
        let endpoint = BrowserEndpoint::from_active_port_file(
            &format!("{}\n/devtools/browser/mock-browser", address.port()),
            listener,
        )
        .unwrap();
        let started = Instant::now();
        assert!(matches!(
            CdpTransport::connect(endpoint),
            Err(TransportError::Timeout)
        ));
        assert!(started.elapsed() < Duration::from_secs(5));
        worker.join().unwrap();
    }

    #[test]
    fn synthetic_loopback_endpoint_runs_only_the_constrained_protocol() {
        use std::net::TcpListener;
        let server = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = server.local_addr().unwrap();
        let pid = std::process::id();
        let worker = std::thread::spawn(move || {
            let (stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let expected = [
                "SystemInfo.getProcessInfo",
                "SystemInfo.getProcessInfo",
                "Target.getTargets",
                "SystemInfo.getProcessInfo",
                "Target.getTargetInfo",
                "Target.attachToTarget",
                "Runtime.evaluate",
                "Target.detachFromTarget",
            ];
            for method in expected {
                let raw = ws.read().unwrap().into_text().unwrap();
                let request: Value = serde_json::from_str(&raw).unwrap();
                assert_eq!(request["method"], method);
                let result = match method {
                    "SystemInfo.getProcessInfo" => {
                        json!({"processInfo":[{"type":"browser","id":pid}]})
                    }
                    "Target.getTargets" => {
                        ws.send(Message::Text(
                            json!({"method":"Target.targetCreated","params":{}})
                                .to_string()
                                .into(),
                        ))
                        .unwrap();
                        json!({"targetInfos":[{"type":"page","targetId":"app-1","url":"https://127.0.0.1:45679/"}]})
                    }
                    "Target.getTargetInfo" => {
                        json!({"targetInfo":{"type":"page","targetId":"app-1","url":"https://127.0.0.1:45679/"}})
                    }
                    "Target.attachToTarget" => {
                        assert_eq!(request["params"]["flatten"], true);
                        json!({"sessionId":"session-1"})
                    }
                    "Runtime.evaluate" => {
                        assert_eq!(request["sessionId"], "session-1");
                        assert_eq!(request["params"]["returnByValue"], true);
                        let script = request["params"]["expression"].as_str().unwrap();
                        assert!(script.contains("location.origin !== \"https://127.0.0.1:45679\""));
                        assert!(script.contains("const forbidden ="));
                        assert!(script.contains("const dictionary = config.dictionary.exact;"));
                        json!({"result":{"value":{"status":"applied","active":true,"translated":1,"awaitingScope":false}}})
                    }
                    "Target.detachFromTarget" => json!({}),
                    _ => unreachable!(),
                };
                ws.send(Message::Text(
                    json!({"id":request["id"],"result":result})
                        .to_string()
                        .into(),
                ))
                .unwrap();
            }
        });
        let listener = VerifiedListener::from_complete_observation(
            address.port(),
            pid,
            &[ListenerObservation {
                address,
                owner_pid: pid,
            }],
        )
        .unwrap();
        let endpoint = BrowserEndpoint::from_active_port_file(
            &format!("{}\n/devtools/browser/mock-browser", address.port()),
            listener,
        )
        .unwrap();
        let mut client = CdpTransport::connect(endpoint).unwrap();
        let pages = client
            .pages(&AppOrigin::from_verified_server_port(45679).unwrap())
            .unwrap();
        let report = client
            .run(&pages[0], "2.19.1", RuntimeAction::Apply)
            .unwrap();
        assert_eq!(report.status, "applied");
        assert!(report.active);
        assert_eq!(report.translated, 1);
        assert!(!report.awaiting_scope);
        worker.join().unwrap();
    }

    #[test]
    fn synthetic_endpoint_with_wrong_browser_pid_is_rejected_before_page_access() {
        use std::net::TcpListener;
        let server = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = server.local_addr().unwrap();
        let pid = std::process::id();
        let worker = std::thread::spawn(move || {
            let (stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().into_text().unwrap().as_str()).unwrap();
            assert_eq!(request["method"], "SystemInfo.getProcessInfo");
            ws.send(Message::Text(json!({"id":request["id"],"result":{"processInfo":[{"type":"browser","id":pid+1}]}}).to_string().into())).unwrap();
            // No target enumeration/evaluation follows a failed identity check.
            assert!(matches!(ws.read(), Ok(Message::Close(_)) | Err(_)));
        });
        let listener = VerifiedListener::from_complete_observation(
            address.port(),
            pid,
            &[ListenerObservation {
                address,
                owner_pid: pid,
            }],
        )
        .unwrap();
        let endpoint = BrowserEndpoint::from_active_port_file(
            &format!("{}\n/devtools/browser/mock-browser", address.port()),
            listener,
        )
        .unwrap();
        assert!(matches!(
            CdpTransport::connect(endpoint),
            Err(TransportError::WrongProcess)
        ));
        worker.join().unwrap();
    }
}
