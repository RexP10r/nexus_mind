#!/usr/bin/env bash

OPTION=""
while [[ $# -gt 0 ]]; do
	case $1 in
		--launch) OPTION="launch"; shift 1 ;; 
		*) shift ;;
	esac
done

log_info()  { echo -e "\033[1;34m[$(date '+%Y-%m-%d %H:%M:%S')] INFO: $*\033[0m" >&2; }
log_error() { echo -e "\033[1;31m[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $*\033[0m" >&2; }
log_success(){ echo -e "\033[1;32m[$(date '+%Y-%m-%d %H:%M:%S')] SUCCESS: $*\033[0m" >&2; }
log_warn()  { echo -e "\033[1;33m[$(date '+%Y-%m-%d %H:%M:%S')] WARN: $*\033[0m" >&2; }
die() { log_error "$*"; exit 1; }

validate_containers() {
	local max_launch_time=30
	local launch_time=0
	log_info "Waiting for all containers to be healthy..."

	until [ -z "$(docker compose ps --format json | jq -r 'select(.Health != "healthy" and .Health != "") | .Name')" ]; do
		printf "."
		launch_time += 2
		if [[ "$launch_time" == "$max_launch_time" ]]; then
			docker compose down
			die "Some container fails"
		fi
		sleep 2
	done
	log_info "All containers are healthy"
}

launch_containers() {
	log_info "Launching data bases containers"
	docker compose up -d

	validate_containers
}

launch_project() {
	launch_containers
}

main() {
	case "$OPTION" in 
		"launch") launch_project ;;
	esac
}

[[ "${BASH_SOURCE[0]}" == "${0}" ]] && main "$@"
