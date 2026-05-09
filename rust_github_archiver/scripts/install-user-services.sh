#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
SERVICE_FILE="$SERVICE_DIR/github-archiver-web.service"
TRUFFLEHOG_BIN="${TRUFFLEHOG_PATH:-$(command -v trufflehog || true)}"
WEB_PORT_DEFAULT="${WEB_PORT:-8081}"

if [[ -z "$TRUFFLEHOG_BIN" ]]; then
  echo "trufflehog not found. Install it or set TRUFFLEHOG_PATH before installing services." >&2
  exit 1
fi

mkdir -p "$SERVICE_DIR"

(
  cd "$PROJECT_DIR"
  cargo build --release --bin web_server
)

cat > "$SERVICE_FILE" <<SERVICE
[Unit]
Description=GitArchiver Web API and dashboard
After=network-online.target docker.service
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$PROJECT_DIR
Environment=WEB_PORT=$WEB_PORT_DEFAULT
Environment=TRUFFLEHOG_PATH=$TRUFFLEHOG_BIN
EnvironmentFile=-$PROJECT_DIR/.env
ExecStart=$PROJECT_DIR/target/release/web_server
Restart=on-failure
RestartSec=5
KillSignal=SIGINT
TimeoutStopSec=30

[Install]
WantedBy=default.target
SERVICE

systemctl --user daemon-reload

if [[ "${1:-}" == "--enable-now" ]]; then
  systemctl --user enable --now github-archiver-web.service
else
  systemctl --user enable github-archiver-web.service
fi

echo "Installed $SERVICE_FILE"
echo "Start now: systemctl --user start github-archiver-web.service"
echo "Status:    systemctl --user status github-archiver-web.service"
