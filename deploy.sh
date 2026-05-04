#!/bin/bash
set -e
cd /opt/colegio-bot
GIT_SSH_COMMAND='ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null' git fetch origin master
LOCAL=$(git rev-parse HEAD)
REMOTE=$(git rev-parse origin/master)
if [ "$LOCAL" != "$REMOTE" ]; then
  echo "$(date): Cambios detectados, haciendo deploy..."
  git stash
  rm -f docker-compose.cima20paas.yml
  git pull origin master
  docker compose -f docker-compose.cima20paas.yml up -d --build
  echo "$(date): Deploy completado"
else
  echo "$(date): Sin cambios"
fi
