#!/usr/bin/env bash
# Pull latest image and recreate app container. DB volume is untouched.

set -euo pipefail

cd "$(dirname "$0")/.."

log() { echo "[$(date -u +%FT%TZ)] $*"; }

if [ -d .git ]; then
  GIT_TERMINAL_PROMPT=0 git pull --ff-only --quiet 2>/dev/null || true
fi

log "pulling latest image"
docker compose pull app

log "recreating app container"
docker compose up -d --no-build app

cid="$(docker compose ps -q app)"
healthy=""
for _ in $(seq 1 40); do
  status="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$cid" 2>/dev/null || echo unknown)"
  case "$status" in
    healthy) healthy=1; break ;;
    unhealthy|exited|dead)
      log "container is $status"
      docker compose logs --tail=30 app
      exit 1
      ;;
  esac
  sleep 3
done

if [ -z "$healthy" ]; then
  log "container never became healthy"
  docker compose logs --tail=30 app
  exit 1
fi

log "app container is healthy"
docker compose logs --tail=5 app
