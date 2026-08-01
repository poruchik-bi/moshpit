#!/bin/sh
# Install and start `mps`, the moshpit server Moshmoni uses for its UDP terminal.
#
#   curl -fsSL https://raw.githubusercontent.com/poruchik-bi/moshpit/android-target-os/install-mps.sh | sh
#
# No root, no Rust toolchain, no prompts: a static binary into ~/.local/bin, a
# host key, a config, and the server running. The one thing left for you is the
# firewall, because that needs sudo and is not a decision a piped script should
# be making — the command is printed at the end.
#
# Run it as the user the app logs in as: `mps` spawns the shell for whoever
# authenticates and reads that user's ~/.mp/authorized_keys.
set -eu

REPO=poruchik-bi/moshpit
ASSET=moshpit-x86_64-linux.tar.gz
BIN="$HOME/.local/bin"
KEY="$HOME/.mp/mps_host_x25519_key"
CONF="$HOME/.config/moshpits/moshpits.toml"
UDP_MIN=50000
UDP_MAX=50009

say() { printf '\033[1m%s\033[0m\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = Linux ] || die "this installs a Linux binary; you are on $(uname -s)"
case "$(uname -m)" in
    x86_64|amd64) ;;
    *) die "no prebuilt binary for $(uname -m) — build the fork: https://github.com/poruchik-bi/moshpit/tree/android-target-os" ;;
esac
command -v curl >/dev/null || die "curl is required"

# ── binaries ────────────────────────────────────────────────────────────────
# Statically linked, so there is no glibc version to match and no toolchain to
# install. Built from this branch — upstream's releases do not carry the
# Android fix, the configurable port range or `advertise_ip`.
say "Installing mps and mp-keygen into $BIN"
mkdir -p "$BIN"
curl -fsSL "https://github.com/$REPO/releases/latest/download/$ASSET" \
    | tar xz -C "$BIN" mps mp-keygen
chmod +x "$BIN/mps" "$BIN/mp-keygen"

# ── host key ────────────────────────────────────────────────────────────────
# mps looks for exactly this path and will not create it itself.
if [ -f "$KEY" ]; then
    say "Host key already present, keeping it"
else
    say "Generating the server host key"
    mkdir -p "$HOME/.mp" && chmod 700 "$HOME/.mp"
    "$BIN/mp-keygen" generate --server --no-passphrase --output-path "$KEY" >/dev/null
fi

# ── config ──────────────────────────────────────────────────────────────────
# advertise_ip is the address the phone sends UDP to, and it cannot be guessed
# from the bind address: the key exchange arrives through an SSH tunnel, so the
# server's own view of the connection is loopback. SSH_CONNECTION already knows
# the address you reached this box on, which is exactly the right answer.
if [ -f "$CONF" ]; then
    say "Config already present, keeping it: $CONF"
else
    IP=$(printf '%s' "${SSH_CONNECTION:-}" | awk '{print $3}')
    [ -n "$IP" ] || IP=$(curl -fsS --max-time 5 https://api.ipify.org 2>/dev/null || true)
    [ -n "$IP" ] || die "could not work out this server's public IP — write $CONF by hand,
  with advertise_ip set to the address your phone reaches this box on"
    say "Writing $CONF, advertising $IP"
    mkdir -p "$(dirname "$CONF")"
    cat > "$CONF" <<EOF
# Written by install-mps.sh. The key exchange stays on loopback and the app
# tunnels to it over SSH, so only the UDP range has to be open.
[mps]
ip           = "127.0.0.1"
port         = 40404
udp_port_min = $UDP_MIN
udp_port_max = $UDP_MAX
advertise_ip = "$IP"
EOF
fi

# ── run it ──────────────────────────────────────────────────────────────────
listening() { ss -ltnH 2>/dev/null | grep -q '127\.0\.0\.1:40404'; }

if listening; then
    say "mps is already listening on 127.0.0.1:40404"
else
    say "Starting mps"
    # `setsid` so it outlives the ssh session this was piped into.
    setsid nohup "$BIN/mps" >"$HOME/.mp/mps.out" 2>&1 &
    # Wait for the socket, not for the process: a server that started and then
    # failed to bind is not a success, and `$!` cannot tell the difference.
    i=0
    while [ $i -lt 10 ]; do
        listening && break
        i=$((i + 1))
        sleep 1
    done
    listening || die "mps did not start listening — see $HOME/.mp/mps.out"
fi

cat <<EOF

$(say "Done.") One thing left, and it needs sudo:

    sudo ufw allow $UDP_MIN:$UDP_MAX/udp

That UDP range is the only inbound port needed — the key exchange goes through
the SSH connection the app already has. One port per concurrent session.

Nothing else to configure: open a session in Moshmoni and it will install its own
key here and say "moshpit" in the status line. If it doesn't, you get the SSH
terminal instead and nothing is broken.

mps is running now but will not survive a reboot. To make it stick:

    (crontab -l 2>/dev/null; echo "@reboot $BIN/mps") | crontab -
EOF

case ":$PATH:" in
    *":$BIN:"*) ;;
    *) printf '\nNote: %s is not on your PATH.\n' "$BIN" ;;
esac
