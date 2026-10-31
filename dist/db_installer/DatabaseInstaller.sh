#!/bin/sh
# Starts the database installer (GUI, or console with --console).
cd "$(dirname "$0")" || exit 1
for exe in ./l2-db-installer ../../target/release/l2-db-installer; do
    if [ -x "$exe" ]; then
        exec "$exe" --sql "$(pwd)/sql" "$@"
    fi
done
echo "l2-db-installer not found. Build it: cargo build --release -p l2-db-installer" >&2
exit 1
