#!/bin/sh
set -e

# If DATABASE_URL is a postgres:// URL, skip SQLite initialization
case "${DATABASE_URL:-}" in
    postgres://*|postgresql://*)
        echo "Using PostgreSQL: ${DATABASE_URL%%@*}@***"
        ;;
    *)
        # SQLite: ensure data directory and database exist
        # Strip sqlite:// prefix to get the file path
        DB_URL="${DATABASE_URL:-sqlite:///data/synodic.db}"
        DB_PATH="${DB_URL#sqlite://}"
        DB_DIR="$(dirname "$DB_PATH")"
        mkdir -p "$DB_DIR"
        if [ ! -f "$DB_PATH" ]; then
            echo "Initializing SQLite database at $DB_PATH"
            sqlite3 "$DB_PATH" "SELECT 1;" > /dev/null
        fi
        export DATABASE_URL="sqlite://$DB_PATH"
        ;;
esac

exec synodic "$@"
