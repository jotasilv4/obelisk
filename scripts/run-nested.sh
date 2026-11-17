#!/usr/bin/env bash
# Desenvolvimento: roda o Obelisk como janela dentro da sua sessão atual (não a substitui).
set -euo pipefail
cd "$(dirname "$0")/.."
exec cargo run -p obelisk-comp -- --command "${TERMINAL:-foot}" "$@"
