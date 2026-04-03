#!/usr/bin/env bash
# Fin — Blueprint Workflow E2E Script
# Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

set -Eeuo pipefail

MODE="offline"
AUTO_FIX=1
ONLINE_STEPS=20
KEEP_TMP=0
ONLINE_MODEL=""

usage() {
  cat <<'EOF'
Usage: scripts/e2e_blueprint.sh [options]

Runs end-to-end blueprint workflow checks with rich logging/debug artifacts.

Modes:
  --mode offline   Deterministic full lifecycle without LLM calls (default)
  --mode online    Live LLM-backed workflow loop (requires provider credentials)
  --mode both      Run offline first, then online

Options:
  --online-steps N   Max fin next iterations in online mode (default: 20)
  --online-model ID  Force model for online mode (e.g. claude-sonnet-4-6)
  --no-auto-fix      Disable self-healing retries for common setup errors
  --keep-tmp         Keep temporary project dir even on success
  -h, --help         Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      MODE="${2:-}"
      shift 2
      ;;
    --online-steps)
      ONLINE_STEPS="${2:-}"
      shift 2
      ;;
    --online-model)
      ONLINE_MODEL="${2:-}"
      shift 2
      ;;
    --no-auto-fix)
      AUTO_FIX=0
      shift
      ;;
    --keep-tmp)
      KEEP_TMP=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ "$MODE" != "offline" && "$MODE" != "online" && "$MODE" != "both" ]]; then
  echo "Invalid --mode '$MODE' (expected offline|online|both)" >&2
  exit 2
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_ARTIFACT_ROOT="${TMPDIR:-/tmp}/fin-e2e-artifacts"
ARTIFACT_ROOT="${FIN_E2E_ARTIFACT_ROOT:-$DEFAULT_ARTIFACT_ROOT}"
RUN_ID="$(date +%Y%m%d-%H%M%S)"
RUN_DIR="$ARTIFACT_ROOT/$RUN_ID"
mkdir -p "$RUN_DIR"
LOG="$RUN_DIR/run.log"
SUMMARY="$RUN_DIR/summary.txt"

FIN_BIN="$REPO_ROOT/target/debug/fin"
if [[ ! -x "$FIN_BIN" ]]; then
  echo "[e2e] building fin binary..." | tee -a "$LOG"
  (cd "$REPO_ROOT" && cargo build -q) >>"$LOG" 2>&1
fi
if [[ ! -x "$FIN_BIN" ]]; then
  echo "fin binary not found at $FIN_BIN" >&2
  exit 1
fi

CURRENT_TMP=""
CURRENT_NAME=""
CURRENT_HOME=""
FAILED=0
SUPPRESS_ERR=0

log() {
  echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG"
}

command_log_path() {
  local name="$1"
  local idx
  idx="$(printf '%03d' "$2")"
  echo "$RUN_DIR/commands/${CURRENT_NAME}-${idx}-${name}.log"
}

dump_debug_bundle() {
  local target="$RUN_DIR/debug/${CURRENT_NAME}"
  mkdir -p "$target"

  {
    echo "=== pwd ==="
    pwd
    echo
    echo "=== env (filtered) ==="
    env | sort | rg '^(FIN_|OPENAI_|ANTHROPIC_|GOOGLE_|GEMINI_|AWS_|OLLAMA_)' || true
    echo
    echo "=== tree (.fin) ==="
    if [[ -d ".fin" ]]; then
      find .fin -maxdepth 6 -print | sort
    else
      echo ".fin missing"
    fi
    echo
    echo "=== git status ==="
    git status --short || true
    echo
    echo "=== fin status ==="
    "$FIN_BIN" status || true
  } > "$target/debug.txt" 2>&1

  if [[ -d ".fin" ]]; then
    cp -R .fin "$target/.fin.snapshot" || true
  fi

  log "debug bundle written: $target"
}

auto_fix_once() {
  local cmd="$1"
  local output_file="$2"
  if [[ "$AUTO_FIX" -ne 1 ]]; then
    return 1
  fi

  if rg -q "No \.fin/ directory found" "$output_file"; then
    log "auto-fix: missing .fin detected, running fin init then retrying"
    "$FIN_BIN" init >>"$LOG" 2>&1 || true
    eval "$cmd"
    return $?
  fi

  return 1
}

CMD_COUNTER=0
run_cmd() {
  local name="$1"
  shift
  local cmd="$*"
  CMD_COUNTER=$((CMD_COUNTER + 1))
  local out
  out="$(command_log_path "$name" "$CMD_COUNTER")"
  mkdir -p "$(dirname "$out")"

  log "run: $cmd"
  SUPPRESS_ERR=1
  set +e
  eval "$cmd" >"$out" 2>&1
  local rc=$?
  set -e
  SUPPRESS_ERR=0
  if [[ $rc -eq 0 ]]; then
    cat "$out" >>"$LOG"
    return 0
  fi

  cat "$out" >>"$LOG"
  log "command failed: $cmd"
  if auto_fix_once "$cmd" "$out"; then
    log "retry succeeded after auto-fix"
    return 0
  fi
  return 1
}

run_expect_fail() {
  local name="$1"
  shift
  local cmd="$*"
  CMD_COUNTER=$((CMD_COUNTER + 1))
  local out
  out="$(command_log_path "$name" "$CMD_COUNTER")"
  mkdir -p "$(dirname "$out")"

  log "run (expect fail): $cmd"
  SUPPRESS_ERR=1
  set +e
  eval "$cmd" >"$out" 2>&1
  local rc=$?
  set -e
  SUPPRESS_ERR=0
  if [[ $rc -eq 0 ]]; then
    cat "$out" >>"$LOG"
    log "expected failure but command succeeded: $cmd"
    return 1
  fi
  cat "$out" >>"$LOG"
  return 0
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  if [[ "$haystack" != *"$needle"* ]]; then
    log "assert failed: expected to find '$needle'"
    return 1
  fi
}

assert_file() {
  local path="$1"
  [[ -f "$path" ]] || {
    log "assert failed: missing file $path"
    return 1
  }
}

has_provider_creds() {
  [[ -n "${ANTHROPIC_API_KEY:-}" ]] \
    || [[ -n "${OPENAI_API_KEY:-}" ]] \
    || [[ -n "${OPENAI_ACCESS_TOKEN:-}" ]] \
    || [[ -n "${OPENAI_BEARER_TOKEN:-}" ]] \
    || [[ -n "${GOOGLE_API_KEY:-}" ]] \
    || [[ -n "${GEMINI_API_KEY:-}" ]] \
    || [[ -n "${GOOGLE_CLOUD_PROJECT:-}" ]] \
    || [[ -n "${CLOUDSDK_CORE_PROJECT:-}" ]] \
    || [[ -n "${AWS_ACCESS_KEY_ID:-}" ]] \
    || [[ -n "${AWS_PROFILE:-}" ]]
}

model_for_provider() {
  local provider="$1"
  case "$provider" in
    anthropic) echo "claude-sonnet-4-6" ;;
    google) echo "gemini-2.5-flash" ;;
    openai) echo "gpt-4.1" ;;
    vertex) echo "claude-sonnet-4@20250514" ;;
    bedrock) echo "anthropic.claude-sonnet-4-20250514-v1:0" ;;
    *) return 1 ;;
  esac
}

detect_provider_for_model() {
  local model="$1"
  case "$model" in
    claude-*@* ) echo "vertex" ;;
    anthropic.* ) echo "bedrock" ;;
    claude-* ) echo "anthropic" ;;
    gemini-* ) echo "google" ;;
    gpt-*|o* ) echo "openai" ;;
    * ) echo "unknown" ;;
  esac
}

provider_is_usable() {
  local provider="$1"
  case "$provider" in
    anthropic) [[ -n "${ANTHROPIC_API_KEY:-}" ]] ;;
    google) [[ -n "${GOOGLE_API_KEY:-}" || -n "${GEMINI_API_KEY:-}" ]] ;;
    openai) [[ -n "${OPENAI_ACCESS_TOKEN:-}" || -n "${OPENAI_BEARER_TOKEN:-}" || -n "${OPENAI_API_KEY:-}" ]] ;;
    vertex) [[ -n "${GOOGLE_CLOUD_PROJECT:-}" || -n "${CLOUDSDK_CORE_PROJECT:-}" ]] ;;
    bedrock) [[ -n "${AWS_ACCESS_KEY_ID:-}" || -n "${AWS_PROFILE:-}" ]] ;;
    *) return 1 ;;
  esac
}

choose_default_online_model() {
  # Prefer non-OpenAI first to avoid common quota/billing failures on personal OPENAI keys.
  for p in anthropic google openai vertex bedrock; do
    if provider_is_usable "$p"; then
      model_for_provider "$p"
      return 0
    fi
  done
  return 1
}

fallback_model() {
  local current_provider="$1"
  for p in anthropic google openai vertex bedrock; do
    if [[ "$p" == "$current_provider" ]]; then
      continue
    fi
    if provider_is_usable "$p"; then
      model_for_provider "$p"
      return 0
    fi
  done
  return 1
}

new_workspace() {
  local name="$1"
  CURRENT_NAME="$name"
  CURRENT_TMP="$(mktemp -d "${TMPDIR:-/tmp}/fin-e2e-${name}.XXXXXX")"
  CURRENT_HOME="$CURRENT_TMP/fin-home"
  export FIN_HOME="$CURRENT_HOME"
  mkdir -p "$CURRENT_HOME"

  cd "$CURRENT_TMP"
  git init -q
  git config user.name "fin-e2e"
  git config user.email "fin-e2e@example.com"
  cat > README.md <<'EOF'
# e2e workspace
EOF
  git add README.md
  git commit -q -m "chore: seed e2e workspace"
  log "workspace($name): $CURRENT_TMP"
}

cleanup_workspace() {
  if [[ -n "$CURRENT_TMP" && -d "$CURRENT_TMP" ]]; then
    if [[ "$KEEP_TMP" -eq 1 || "$FAILED" -eq 1 ]]; then
      log "keeping workspace: $CURRENT_TMP"
    else
      rm -rf "$CURRENT_TMP"
    fi
  fi
}

offline_e2e() {
  log "=== offline e2e start ==="
  new_workspace "offline"
  CMD_COUNTER=0

  run_cmd init "\"$FIN_BIN\" init"
  run_cmd status_initial "\"$FIN_BIN\" status"

  local status_text
  status_text="$("$FIN_BIN" status 2>&1)"
  assert_contains "$status_text" "Active Blueprint:" || return 1
  assert_contains "$status_text" "None" || return 1

  run_cmd blueprint_new "\"$FIN_BIN\" blueprint new \"E2E Blueprint\""
  assert_file ".fin/blueprints/B001/B001-VISION.md"
  assert_file ".fin/STATUS.md"

  run_cmd blueprint_list "\"$FIN_BIN\" blueprint list"

  run_cmd blueprint_blocked "\"$FIN_BIN\" blueprint new \"Should Be Blocked\""
  local blocked_text
  blocked_text="$("$FIN_BIN" blueprint new "Should Be Blocked 2" 2>&1 || true)"
  assert_contains "$blocked_text" "already in progress" || return 1

  run_expect_fail complete_should_fail "\"$FIN_BIN\" blueprint complete"

  mkdir -p ".fin/blueprints/B001/sections/S01/tasks"
  cat > ".fin/blueprints/B001/sections/S01/S01-REPORT.md" <<'EOF'
# S01 Report

Section completed in deterministic offline test.
EOF

  run_cmd complete_success "\"$FIN_BIN\" blueprint complete"

  local final_status
  final_status="$("$FIN_BIN" status 2>&1)"
  assert_contains "$final_status" "COMPLETE" || return 1

  run_cmd blueprint_new_after_complete "\"$FIN_BIN\" blueprint new \"B002 Follow-up\""
  assert_file ".fin/blueprints/B002/B002-VISION.md"

  cleanup_workspace
  log "=== offline e2e passed ==="
}

online_e2e() {
  log "=== online e2e start ==="
  if ! has_provider_creds; then
    log "online e2e skipped: no provider credentials found"
    return 0
  fi

  new_workspace "online"
  CMD_COUNTER=0

  run_cmd init "\"$FIN_BIN\" init"
  run_cmd blueprint_new "\"$FIN_BIN\" blueprint new \"Online E2E Blueprint\""

  local current_model="$ONLINE_MODEL"
  if [[ -z "$current_model" ]]; then
    current_model="$(choose_default_online_model || true)"
  fi
  local current_provider="default"
  if [[ -n "$current_model" ]]; then
    current_provider="$(detect_provider_for_model "$current_model")"
    log "online e2e using model override: $current_model (provider: $current_provider)"
  else
    log "online e2e using fin default model selection"
  fi

  local i
  for ((i=1; i<=ONLINE_STEPS; i++)); do
    local next_cmd
    if [[ -n "$current_model" ]]; then
      next_cmd="\"$FIN_BIN\" --model \"$current_model\" next"
    else
      next_cmd="\"$FIN_BIN\" next"
    fi

    if ! run_cmd "next_${i}" "$next_cmd"; then
      return 1
    fi

    local next_log
    next_log="$(command_log_path "next_${i}" "$CMD_COUNTER")"
    local step_out
    step_out="$(cat "$next_log" 2>/dev/null || true)"

    if [[ "$step_out" == *"Outcome: Error("* || "$step_out" == *"Workflow error:"* ]]; then
      if [[ "$step_out" == *"insufficient_quota"* || "$step_out" == *"returned 429"* ]]; then
        local fb
        fb="$(fallback_model "$current_provider" || true)"
        if [[ -n "$fb" ]]; then
          current_model="$fb"
          current_provider="$(detect_provider_for_model "$current_model")"
          log "quota/rate error detected; switching to fallback model: $current_model"
          continue
        fi
        log "online e2e failing fast: provider quota/rate error and no fallback credentials available"
        return 1
      fi
      log "online e2e failing fast: workflow reported error outcome"
      return 1
    fi

    local status_text
    status_text="$("$FIN_BIN" status 2>&1 || true)"
    if [[ "$status_text" == *"COMPLETE"* ]]; then
      log "online e2e complete after $i step(s)"
      cleanup_workspace
      log "=== online e2e passed ==="
      return 0
    fi
  done

  log "online e2e did not reach COMPLETE within $ONLINE_STEPS steps"
  return 1
}

on_error() {
  if [[ "$SUPPRESS_ERR" -eq 1 ]]; then
    return 0
  fi
  FAILED=1
  log "FAIL: e2e workflow failed at line ${BASH_LINENO[0]} in ${CURRENT_NAME:-unknown} mode"
  dump_debug_bundle
  cleanup_workspace
  {
    echo "FAILED"
    echo "run_dir=$RUN_DIR"
    echo "log=$LOG"
  } > "$SUMMARY"
}
trap on_error ERR

{
  echo "run_id=$RUN_ID"
  echo "mode=$MODE"
  echo "run_dir=$RUN_DIR"
} > "$SUMMARY"

mkdir -p "$RUN_DIR/commands" "$RUN_DIR/debug"

case "$MODE" in
  offline)
    offline_e2e
    ;;
  online)
    online_e2e
    ;;
  both)
    offline_e2e
    online_e2e
    ;;
esac

echo "PASSED" > "$SUMMARY"
echo "run_dir=$RUN_DIR" >> "$SUMMARY"
echo "log=$LOG" >> "$SUMMARY"
log "all requested e2e modes passed"
