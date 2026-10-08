//! Reuse Antigravity App's existing local debugging listener, without launching
//! it or changing its bundle, credentials or security settings.
#[cfg(target_os = "macos")]
pub(crate) fn connect() -> Result<
    (
        super::app_transport::CdpTransport,
        Vec<super::app_transport::PageTarget>,
        String,
    ),
    String,
> {
    use super::{app_metadata_macos as metadata, app_transport::*};
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let config = super::config::load_app_config()?;
    let installation = metadata::installed(config.antigravity_executable.as_deref())?
        .ok_or("app_not_installed")?;
    let pids = metadata::pids(&[
        "-u".into(),
        unsafe { libc::geteuid() }.to_string(),
        "-x".into(),
        "Antigravity".into(),
    ])?;
    let pid = metadata::verified_app_pid(&pids, |pid| {
        metadata::verify_executable(pid, &installation.executable)
    })?;
    let path = dirs::home_dir()
        .ok_or("no_debug_port")?
        .join("Library/Application Support/Antigravity/DevToolsActivePort");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "no_debug_port")?;
    if !file.metadata().map_err(|_| "no_debug_port")?.is_file() {
        return Err("no_debug_port".into());
    }
    let mut contents = String::new();
    file.take(257)
        .read_to_string(&mut contents)
        .map_err(|_| "no_debug_port")?;
    let port = contents
        .lines()
        .next()
        .and_then(|v| v.parse::<u16>().ok())
        .ok_or("no_debug_port")?;
    metadata::listener(port, pid)?;
    let rows = metadata::parse_listener_fields(&metadata::command(
        "/usr/sbin/lsof",
        &[
            "-nP".into(),
            format!("-iTCP:{port}"),
            "-sTCP:LISTEN".into(),
            "-Fpn".into(),
        ],
    )?)?;
    let rows = rows
        .into_iter()
        .map(|r| ListenerObservation {
            address: r.address,
            owner_pid: r.owner_pid,
        })
        .collect::<Vec<_>>();
    let endpoint = BrowserEndpoint::from_active_port_file(
        &contents,
        VerifiedListener::from_complete_observation(port, pid, &rows)
            .map_err(|_| "no_debug_port")?,
    )
    .map_err(|_| "no_debug_port")?;
    let servers = metadata::pids(&[
        "-P".into(),
        pid.to_string(),
        "-x".into(),
        "language_server".into(),
    ])?;
    let mut origins = vec![];
    for server in servers {
        metadata::verify_executable(
            server,
            &installation
                .bundle
                .join("Contents/Resources/bin/language_server"),
        )?;
        let rows = metadata::parse_listener_fields(&metadata::command(
            "/usr/sbin/lsof",
            &[
                "-nP".into(),
                "-a".into(),
                "-p".into(),
                server.to_string(),
                "-iTCP".into(),
                "-sTCP:LISTEN".into(),
                "-Fpn".into(),
            ],
        )?)?;
        for port in rows
            .iter()
            .map(|r| r.address.port())
            .collect::<std::collections::BTreeSet<_>>()
        {
            if metadata::listener(port, server).is_ok() {
                origins.push(
                    AppOrigin::from_verified_server_port(port).map_err(|_| "identity_mismatch")?,
                );
            }
        }
    }
    metadata::verify_executable(pid, &installation.executable)?;
    let mut connection = CdpTransport::connect(endpoint).map_err(|_| "app_connection_failed")?;
    let pages = connection
        .pages_for_origins(&origins)
        .map_err(|_| "app_page_unavailable")?;
    Ok((connection, pages, installation.version))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn connect() -> Result<
    (
        super::app_transport::CdpTransport,
        Vec<super::app_transport::PageTarget>,
        String,
    ),
    String,
> {
    Err("unsupported_platform".into())
}
