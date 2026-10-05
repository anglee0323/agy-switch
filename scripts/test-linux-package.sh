#!/usr/bin/env bash
# Fresh hosted VM only; install the real deb and exercise read-only commands.
set -euo pipefail
[[ "${GITHUB_ACTIONS:-}" == true && "${RUNNER_ENVIRONMENT:-}" == github-hosted ]] || { echo 'Use a fresh GitHub-hosted runner' >&2; exit 1; }
[[ $# == 1 ]] || { echo 'Usage: test-linux-package.sh PACKAGE.deb' >&2; exit 2; }
task_package="$(realpath "$1")"
task_name="$(dpkg-deb -f "$task_package" Package)"
[[ "$task_name" == agy-switch ]]
if dpkg-query -W -f='${Status}' "$task_name" 2>/dev/null | grep -q 'install ok installed'; then
  echo 'Refusing an existing installation' >&2; exit 1
fi
for task_binary in /usr/bin/agy-switch-desktop /usr/bin/agy-switch; do
  [[ ! -e "$task_binary" ]] || { echo 'Refusing an existing executable' >&2; exit 1; }
done
task_installed=false
cleanup() { if [[ "$task_installed" == true ]]; then sudo apt-get remove -y "$task_name"; fi; }
trap cleanup EXIT
sudo apt-get install --no-install-recommends -y "$task_package"
task_installed=true
node scripts/test-cli.mjs /usr/bin/agy-switch-desktop
node scripts/test-cli.mjs /usr/bin/agy-switch
echo 'Actual APT installation and both installed read-only executables passed; no GUI/auth/account writes'
