#!/usr/bin/env bash
set -euo pipefail

env="${TASKLIST_PROJECTS:-code247.logline.world llm-gateway.logline.world logic.logline.world obs-api.logline.world 'Research Lab/voulezvous-tv-codex' VoulezvousPlataforma}"
IFS='\n'
for project in $env; do
  echo "---"
  echo "Project: $project"
  path="$project/TASKLIST.md"
  if [[ -f $path ]]; then
    echo
    sed -n '1,200p' "$path"
  else
    echo "No TASKLIST.md found"
  fi
  echo
done
