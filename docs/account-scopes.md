# Account switching and local import

Manual switching defaults to Antigravity App and the initialized native agy CLI
session together. Both clients can use the same OS credential store. Existing
long-running agy commands can retain an old session; start a new command after
switching. IDE switching updates only the IDE databases and device data, closes
and reopens only a running IDE, and keeps App/CLI and plugin credentials untouched.

On macOS, manual switching also stops an independently running, verified App-data
Hub before writing credentials and restarts it before reopening the App. Otherwise
that process can retain the previous session after the window closes. Unverifiable
Hub ownership or a failed close blocks the write. This does not establish live
switching acceptance on every Antigravity version or platform.

The account page and native menu switch App and CLI together without a client
selector. Explicit IDE switching remains available through the management CLI
and smart-switch policy. Selections are recorded separately; a saved selection
does not prove a client's live identity.
`agy-switch switch ID --target app|ide` uses the same service.
`agy-switch current --target app|ide --json` reads the selected client's saved
record without accessing live credentials.

New automatic-switch policies default to App/CLI. Existing explicit global
policies retain their scope; choose App/CLI or IDE in smart-switch settings to
isolate them. The scheduler validates the selected client's credentials before
closing or switching it.

Local import scans App, CLI, IDE and supported plugin sources independently.
IDE discovery includes space, hyphen and underscore directory names and running
or configured portable installations. The custom database picker covers other
user-data directories. Discovery reads SQLite without changing the client data.

App startup arguments use `antigravity_args`; IDE arguments use the separate
`antigravity_ide_args` setting. Discovery preserves case in running-process paths
and reads each client's `--user-data-dir` independently. Independent switching
rejects a shared database before closing either client; configure separate data
directories if both clients explicitly use the same one.

Import validates each source through Google before saving it and reports failed,
missing and duplicate sources. A failed source does not prevent a later source
from importing the same token. App keyring credentials are never used as a fallback
for an IDE database. Expired credentials or unavailable networks remain visible
in the import results instead of being silently skipped.

After a partial token/JSON import, the dialog stays open for reviewing the result.
Cancel or Confirm closes it without repeating successful imports.

Account plan filters and list/card badges use the same classification. Missing,
null or blank subscription tiers appear as Unknown instead of Free, and the Unknown
filter is shown when needed. All = Pro + Ultra + Free + Unknown; each account belongs
to exactly one category. OAuth login already attempts a quota refresh; a failed or
incomplete plan observation stays unknown across restarts until a later refresh
reports it.

## Export and restore

JSON export is the last action for each account and in the selected-account
toolbar, after the regular account controls. The suggested filename includes the
local date and time to the second, for example
`antigravity-accounts-2026-10-12_03-05-00.json`. Version 2 exports
preserve refresh tokens, OAuth client/profile metadata, project ID, name and remark.
They omit access tokens, device identifiers and quota caches. On Unix the saved
file uses mode 0600. The add-account dialog accepts that file, older token-only JSON
and pasted refresh tokens; restored accounts are validated before saving.
Exported files contain login credentials and should be kept private.

The local-import file picker accepts JSON exports as well as SQLite databases.
JSON batches are validated before importing; partial failures are counted and
remain visible instead of being reported as complete success. The exported OAuth
client key is used when validating restored credentials.

## Network routing

Network settings default to the system proxy. Explicit HTTP/HTTPS/ALL_PROXY
environment variables take precedence. macOS system fixed proxies, Windows Internet
Settings, GNOME desktop fixed proxies and KDE kioslaverc fixed proxies are read
when constructing a client. Google requests share a short-lived client pool; proxy
changes take effect on new requests after at most two seconds. Loopback App RPC
remains direct.

Settings also provide manual HTTP, HTTPS, SOCKS5/SOCKS5h routing and direct mode.
Mode changes apply automatically. A valid manual address applies after a short
typing pause, on leaving the field or on Enter; incomplete or invalid addresses
retain the previously applied routing. No separate Save action is needed.
PAC scripts and automatic proxy discovery are not evaluated; users with those
settings receive a message to enter the proxy tool's local HTTP or SOCKS address.
Other Linux desktops can use environment variables or the manual setting. TUN is
not required for fixed-proxy routing. New settings apply to authorization,
refresh-token import, quota requests, pricing and update downloads.
