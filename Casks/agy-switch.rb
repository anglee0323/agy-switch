# Generated from a release archive; do not replace SHA-256 with :no_check.
cask "agy-switch" do
  version "4.9.3"
  sha256 "6ceabf7e6aa8761a0f8bd2dfb8cbeebc56ea827446c150f48e6b6b8ceedc8e78"

  url "https://github.com/anglee0323/agy-switch/releases/download/v4.9.3/agy-switch-4.9.3-macos-arm64.zip"
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
