# Generated from a release archive; do not replace SHA-256 with :no_check.
cask "agy-switch" do
  version "4.8.1"
  sha256 "e36a70b759c2ecbff6d3863dcc7a4a5aa4673295c7e92313b3538f196e141dc0"

  url "https://github.com/anglee0323/agy-switch/releases/download/v4.8.1/Antigravity-Tools-Lite-4.8.1-macos-arm64.zip"
  name "agy-switch"
  desc "Antigravity account manager, local usage dashboard and agy-switch CLI"
  homepage "https://github.com/anglee0323/agy-switch"

  depends_on arch: :arm64
  depends_on :macos

  app "Antigravity Tools Lite.app"
  binary "#{appdir}/Antigravity Tools Lite.app/Contents/MacOS/antigravity-tools", target: "agy-switch"

  caveats <<~EOS
    Includes the agy-switch command. Run agy-switch --help to get started.
    This management CLI is separate from Google's agy command.
    Account data and system credentials are retained when uninstalling.
  EOS
end
