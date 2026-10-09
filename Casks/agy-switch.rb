# Generated from a release archive; do not replace SHA-256 with :no_check.
cask "agy-switch" do
  version "4.10.1"
  sha256 "430d318de34e88621e503b12f8408658830ce55bb00964a0155eb4cf6eeca744"

  url "https://github.com/anglee0323/agy-switch/releases/download/v4.10.1/agy-switch-4.10.1-macos-arm64.zip"
  name "AntiGravity Switch"
  desc "Antigravity account manager, local usage dashboard and agy-switch CLI"
  homepage "https://github.com/anglee0323/agy-switch"

  depends_on arch: :arm64
  depends_on :macos

  app "AntiGravity Switch.app"
  binary "#{appdir}/AntiGravity Switch.app/Contents/MacOS/agy-switch-desktop", target: "agy-switch"

  caveats <<~EOS
    Includes the agy-switch command. Run agy-switch --help to get started.
    This management CLI is separate from Google's agy command.
    Account data and system credentials are retained when uninstalling.
  EOS
end
