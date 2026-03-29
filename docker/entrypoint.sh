#!/bin/sh
set -e

# If DATABASE_URL is a postgres:// URL, skip SQLite initialization
case "${DATABASE_URL:-}" in
    postgres://*|postgresql://*)
        echo "Using PostgreSQL: ${DATABASE_URL%%@*}@***"
        ;;
    *)
        # SQLite: ensure data directory and database exist
        DB_PATH="${DATABASE_URL:-/data/synodic.db}"
        DB_DIR="$(dirname "$DB_PATH")"
        mkdir -p "$DB_DIR"
        if [ ! -f "$DB_PATH" ]; then
            echo "Initializing SQLite database at $DB_PATH"
            sqlite3 "$DB_PATH" "SELECT 1;" > /dev/null
        fi
        export DATABASE_URL="$DB_PATH"
        ;;
esac

exec synodic-http "$@"
