#!/bin/sh
# L2 Rust server setup for Linux: checks the tools, builds the programs, then runs l2-setup,
# which makes the server folder and sets up the database (new, or an existing L2JMobius H5
# database). Any arguments go on to l2-setup (relative paths are relative to the folder you
# start it from; ./target/release/l2-setup --help lists them). Nothing is installed without
# asking first.
set -e
repo="$(cd "$(dirname "$0")" && pwd)"

ask() {
    printf '%s (y/n): ' "$1"
    read -r answer
    case "$answer" in y|Y|yes|j|ja) return 0 ;; *) return 1 ;; esac
}

echo "== L2 Rust server: build and setup =="

# 1. Rust (rustup installs the toolchain of rust-toolchain.toml by itself).
[ -x "$HOME/.cargo/bin/cargo" ] && PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust is not installed."
    if command -v curl >/dev/null 2>&1 && ask "Install Rust (rustup, from https://sh.rustup.rs) now?"; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        PATH="$HOME/.cargo/bin:$PATH"
    fi
    if ! command -v cargo >/dev/null 2>&1; then
        echo "Install Rust from https://rustup.rs and run ./setup.sh again."
        exit 1
    fi
fi

# 2. A C compiler and linker (Rust needs one).
if ! command -v cc >/dev/null 2>&1; then
    echo "No C compiler (cc) found. Install it first, for example:"
    echo "  Debian/Ubuntu: sudo apt install build-essential pkg-config"
    echo "  Fedora:        sudo dnf install gcc pkgconf"
    exit 1
fi

# 3. MariaDB or MySQL (only checked; the setup asks for the connection later).
client=""
command -v mariadb >/dev/null 2>&1 && client="mariadb"
[ -z "$client" ] && command -v mysql >/dev/null 2>&1 && client="mysql"
if [ -z "$client" ] && ! command -v mariadbd >/dev/null 2>&1 && ! command -v mysqld >/dev/null 2>&1; then
    echo "No MariaDB or MySQL was found on this machine (a database on another machine works too)."
    echo "  Debian/Ubuntu: sudo apt install mariadb-server"
    echo "  Fedora:        sudo dnf install mariadb-server && sudo systemctl enable --now mariadb"
elif [ -n "$client" ] && command -v sudo >/dev/null 2>&1; then
    # On Debian/Ubuntu root logs in only through the system's socket, not with a password over
    # the network as the servers do: they need a user of their own.
    if ask "Create a database user for the server now (with sudo $client)?"; then
        printf 'User name [l2]: '
        read -r user
        [ -z "$user" ] && user="l2"
        printf 'Password for %s: ' "$user"
        stty -echo 2>/dev/null || true
        read -r password
        stty echo 2>/dev/null || true
        echo
        escaped=$(printf '%s' "$password" | sed "s/'/''/g")
        sudo "$client" -e "CREATE USER IF NOT EXISTS '$user'@'localhost' IDENTIFIED BY '$escaped'; GRANT ALL PRIVILEGES ON *.* TO '$user'@'localhost';"
        echo "User $user created: give it to the setup below as the database user."
    fi
fi

# 4. Build (from the repository, wherever this was started from).
echo
echo "Building (the first build takes several minutes) ..."
cargo build --release --manifest-path "$repo/Cargo.toml" -p login-server -p game-server -p l2-supervisor -p l2-admin-gui -p l2-db-installer -p l2-setup

# 5. The server folder and the database (relative paths: from the folder this was started in).
exec "$repo/target/release/l2-setup" --repo "$repo" "$@"
