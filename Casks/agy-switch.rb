# Generated from a release archive; do not replace SHA-256 with :no_check.
cask "agy-switch" do
  version "4.9.2"
  sha256 "9546ab80b0112257e5af86b7dd962bc8bc7e7a7a49b0e770f4560f7f82588101"

  url "https://github.com/anglee0323/agy-switch/releases/download/v4.9.2/agy-switch-4.9.2-macos-arm64.zip"
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
