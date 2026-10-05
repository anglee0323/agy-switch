# Generated from a release archive; do not replace SHA-256 with :no_check.
cask "agy-switch" do
  version "4.9.0"
  sha256 "b8799230d6a378a447dbeee654aaf108eff33e696e1378987bd9af243f22b02f"

  url "https://github.com/anglee0323/agy-switch/releases/download/v4.9.0/agy-switch-4.9.0-macos-arm64.zip"
  name "agy-switch"
  desc "Antigravity account manager, local usage dashboard and agy-switch CLI"
  homepage "https://github.com/anglee0323/agy-switch"

  depends_on arch: :arm64
  depends_on :macos

  app "agy-switch.app"
  binary "#{appdir}/agy-switch.app/Contents/MacOS/agy-switch-desktop", target: "agy-switch"

  caveats <<~EOS
    Includes the agy-switch command. Run agy-switch --help to get started.
    This management CLI is separate from Google's agy command.
    Account data and system credentials are retained when uninstalling.
  EOS
end
