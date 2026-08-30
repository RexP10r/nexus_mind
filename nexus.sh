#!/usr/bin/env bash

OPTION=""
while [[ $# -gt 0 ]]; do
	case $1 in
		--launch) OPTION="launch"; shift 1 ;;
		--stop)   OPTION="stop";   shift 1 ;;
		*) shift ;;
	esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
META_DIR="$SCRIPT_DIR/.script_meta"
LM_PID_FILE="$META_DIR/lm_service.pid"

if [[ -f "$SCRIPT_DIR/.env" ]]; then
    set -a
    source "$SCRIPT_DIR/.env"
    set +a
else
    die ".env file not found in $SCRIPT_DIR"
fi

log_info()  { echo -e "\033[1;34m[$(date '+%Y-%m-%d %H:%M:%S')] INFO: $*\033[0m" >&2; }
log_error() { echo -e "\033[1;31m[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $*\033[0m" >&2; }
log_success(){ echo -e "\033[1;32m[$(date '+%Y-%m-%d %H:%M:%S')] SUCCESS: $*\033[0m" >&2; }
log_warn()  { echo -e "\033[1;33m[$(date '+%Y-%m-%d %H:%M:%S')] WARN: $*\033[0m" >&2; }
die() { log_error "$*"; exit 1; }

# ── PID helpers ──────────────────────────────────────────────

init_meta_dir() {
	mkdir -p "$META_DIR"
}

save_pid() {
	local pid="$1"
	local file="$2"
	init_meta_dir
	echo "$pid" > "$file"
	log_info "Saved PID $pid → $file"
}

load_pid() {
	local file="$1"

	if [[ ! -f "$file" ]]; then
		log_warn "PID file not found: $file"
		return 1
	fi

	local pid
	pid="$(cat "$file" 2>/dev/null)"

	if [[ -z "$pid" || ! "$pid" =~ ^[0-9]+$ ]]; then
		log_warn "Invalid PID in $file"
		rm -f "$file"
		return 1
	fi

	if ! kill -0 "$pid" 2>/dev/null; then
		log_warn "Process $pid is no longer running (stale PID file)"
		rm -f "$file"
		return 1
	fi

	echo "$pid"
	return 0
}

remove_pid() {
	rm -f "$1"
}

# ── Containers ───────────────────────────────────────────────

launch_containers() {
	log_info "Launching data bases containers"
	docker compose up -d

	local max_launch_time=30
	local launch_time=0
	log_info "Waiting for all containers to be healthy..."

	until [ -z "$(docker compose ps --format json | jq -r 'select(.Health != "healthy" and .Health != "") | .Name')" ]; do
		printf "."
		((launch_time+=2))
		if [[ "$launch_time" -ge "$max_launch_time" ]]; then
			echo ""
			docker compose down
			die "Some container falls"
		fi
		sleep 2
	done
	echo ""
	log_success "All containers are healthy"
}

# ── Port check ───────────────────────────────────────────────

check_port() {
    python3 -c "
import socket, sys
try:
    s = socket.create_connection((sys.argv[1], int(sys.argv[2])), timeout=1)
    s.close()
    sys.exit(0)
except Exception:
    sys.exit(1)
" "$1" "$2" > /dev/null 2>&1
}

# ── LM service ───────────────────────────────────────────────

launch_lm_service() {
	if load_pid "$LM_PID_FILE" >/dev/null 2>&1; then
		log_info "LM service is already running (PID $(load_pid "$LM_PID_FILE"))"
		return 0
	fi

	mkdir -p logs
	log_info "Launching lm service"

	uv run --directory crates/lm-service main.py > ./logs/lm_service.log 2>&1 &
	local PID=$!
	save_pid "$PID" "$LM_PID_FILE"

	log_info "Check if lm service gRPC is available..."

	local addr="$LM_SERVICE_GRPC_ADDR_CUTTED"
	local host port

	if [[ "$addr" =~ ^\[(.*)\]:([0-9]+)$ ]]; then
		host="${BASH_REMATCH[1]}"
		port="${BASH_REMATCH[2]}"
	elif [[ "$addr" =~ ^(.*):([0-9]+)$ ]]; then
		host="${BASH_REMATCH[1]}"
		port="${BASH_REMATCH[2]}"
	else
		echo ""
		kill "$PID" 2>/dev/null
		remove_pid "$LM_PID_FILE"
		die "Invalid address format: $addr"
	fi

	for i in {1..30}; do
		if check_port "$host" "$port"; then
			echo ""
			log_success "Lm service is healthy"
			return 0
		fi
		if [[ "$i" -eq 30 ]]; then
			echo ""
			kill "$PID" 2>/dev/null
			remove_pid "$LM_PID_FILE"
			die "Lm service failed to start"
		fi
		printf "."
		sleep 1
	done
	echo ""
}

stop_lm_service() {
	local pid
	if ! pid="$(load_pid "$LM_PID_FILE")"; then
		log_warn "LM service is not running"
		return 0
	fi

	log_info "Stopping lm service (PID $pid)..."
	kill "$pid" 2>/dev/null

	local waited=0
	while kill -0 "$pid" 2>/dev/null && [[ $waited -lt 10 ]]; do
		sleep 1
		((waited++))
	done

	if kill -0 "$pid" 2>/dev/null; then
		log_warn "Process did not exit, sending SIGKILL"
		kill -9 "$pid" 2>/dev/null
	fi

	remove_pid "$LM_PID_FILE"
	log_success "Lm service stopped"
}

# ── Orchestration ────────────────────────────────────────────

launch_project() {
	launch_containers
	launch_lm_service
}

stop_project() {
	stop_lm_service
	log_info "Stopping containers..."
	docker compose down
	log_success "All stopped"
}

main() {
	case "$OPTION" in
		"launch") launch_project ;;
		"stop")   stop_project   ;;
		*)        die "Usage: $0 --launch | --stop" ;;
	esac
}

[[ "${BASH_SOURCE[0]}" == "${0}" ]] && main "$@"
