#!/usr/bin/env sh
# Sets pley up with Docker Compose and starts it:
#   ./deploy/docker/setup.sh
# The first run writes .env: pley runs as the user who runs this (or who ran
# sudo), with hardware transcoding on the GPUs Docker can use. Then it checks
# the config and media folders, builds and starts the container, waits until
# pley answers and prints the link that creates the administrator account.
# Run it again after `git pull` to update pley; .env is kept.
set -eu

cd "$(dirname "$0")"

say() { printf '%s\n' "$*"; }
die() {
    printf 'setup.sh: %s\n' "$*" >&2
    exit 1
}

# A setting as Docker Compose sees it: the environment first, then .env,
# then the default.
setting() {
    eval "value=\${$1-}"
    if [ -z "$value" ] && [ -f .env ]; then
        value=$(sed -n "s/^[[:space:]]*$1=//p" .env | tail -n 1 |
            sed -e 's/[[:space:]]*$//' -e 's/^"\(.*\)"$/\1/' -e "s/^'\(.*\)'\$/\1/")
    fi
    printf '%s' "${value:-$2}"
}

command -v docker >/dev/null 2>&1 || die "Docker isn't installed: https://docs.docker.com/engine/install/"
docker compose version >/dev/null 2>&1 ||
    die "Docker Compose isn't installed (Debian and Ubuntu: the docker-compose-plugin package)"
docker info >/dev/null 2>&1 ||
    die "Docker isn't running, or this user may not use it (add it to the docker group, or run this with sudo)"

case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) os=windows sep=';' ;;
    Linux) os=linux sep=':' ;;
    *) os=other sep=':' ;;
esac
# Rootless Docker runs containers as this user: root in the container is this
# user outside it, and other users are ones with no files here.
rootless=
if docker info --format '{{.SecurityOptions}}' | grep -q rootless; then rootless=1; fi

# ---- .env, on the first run ------------------------------------------------
if [ ! -f .env ]; then
    # Docker Desktop (macOS, Windows) maps file owners by itself; on Linux pley
    # runs as whoever owns the folders: this user, or the one who ran sudo.
    uid=1000 gid=1000
    if [ -n "$rootless" ]; then
        uid=0 gid=0
    elif [ "$os" = linux ]; then
        uid=${SUDO_UID:-$(id -u)} gid=${SUDO_GID:-$(id -g)}
        [ "$uid" != 0 ] || uid=1000 gid=1000
    fi
    set -- -e "s|^PLEY_UID=.*|PLEY_UID=$uid|" -e "s|^PLEY_GID=.*|PLEY_GID=$gid|"

    # GPUs, on an engine that runs on this computer (not in Docker Desktop's
    # VM): the first render node of an Intel or AMD GPU (not NVIDIA's, nor
    # WSL's Direct3D bridge). One in group 0 would make pley a member of
    # root's group.
    files=compose.yaml
    render_node=
    if [ "$os" = linux ] && ! docker info --format '{{.OperatingSystem}}' | grep -q 'Docker Desktop'; then
        for node in /dev/dri/renderD*; do
            [ -c "$node" ] || continue
            driver=$(basename "$(readlink -f "/sys/class/drm/${node##*/}/device/driver" 2>/dev/null)")
            case $driver in
                i915 | xe | amdgpu | radeon)
                    render_node=$node
                    break
                    ;;
            esac
        done
    fi
    if [ -n "$render_node" ] && [ "$(stat -c %g "$render_node")" != 0 ]; then
        files="$files${sep}compose.vaapi.yaml"
        set -- "$@" -e "s|^# PLEY_RENDER_GID=.*|PLEY_RENDER_GID=$(stat -c %g "$render_node")|"
        [ "$render_node" = /dev/dri/renderD128 ] ||
            set -- "$@" -e "s|^# PLEY_RENDER_DEVICE=.*|PLEY_RENDER_DEVICE=$render_node|"
        say "Found an Intel or AMD GPU ($driver, $render_node): hardware transcoding is on (compose.vaapi.yaml)."
    fi
    if docker info --format '{{json .Runtimes}}' | grep -q '"nvidia"'; then
        files="$files${sep}compose.nvidia.yaml"
        say "Found the NVIDIA Container Toolkit: NVIDIA hardware transcoding is on (compose.nvidia.yaml)."
    fi
    [ "$files" = compose.yaml ] || set -- "$@" -e "s|^# COMPOSE_FILE=.*|COMPOSE_FILE=$files|"

    sed "$@" .env.example >.env.new
    mv .env.new .env
    [ -z "${SUDO_UID-}" ] || chown "$SUDO_UID:$SUDO_GID" .env
    say "Wrote .env (pley runs as user $uid, group $gid); edit it to change folders or ports."
fi

# ---- folders ---------------------------------------------------------------
config_dir=$(setting PLEY_CONFIG_DIR ./config)
media_dir=$(setting PLEY_MEDIA_DIR ./media)
run_uid=$(setting PLEY_UID 1000)
run_gid=$(setting PLEY_GID 1000)
[ "$run_uid" != 0 ] || [ -n "$rootless" ] || die "PLEY_UID is 0: pley must not run as root"

if [ ! -d "$config_dir" ]; then
    mkdir -p "$config_dir" ||
        die "can't create $config_dir (PLEY_CONFIG_DIR): create it, owned by $run_uid:$run_gid"
    [ "$(id -u)" != 0 ] || chown "$run_uid:$run_gid" "$config_dir"
fi
if [ "$os" = linux ]; then
    # The user pley's files belong to outside the container.
    host_uid=$run_uid host_gid=$run_gid
    [ -z "$rootless" ] || host_uid=$(id -u) host_gid=$(id -g)
    owner=$(ls -nd "$config_dir" | awk '{print $3}')
    if [ "$owner" = "$host_uid" ]; then
        # The database and the key in it are for pley alone.
        chmod 700 "$config_dir" 2>/dev/null || true
    else
        say "Warning: $config_dir belongs to user $owner but pley runs as $host_uid, so it may not be able to"
        say "write there. Fix it with: sudo chown -R $host_uid:$host_gid $config_dir"
    fi
fi
[ -d "$media_dir" ] || die "$media_dir (PLEY_MEDIA_DIR in .env) isn't a folder"

# ---- start -----------------------------------------------------------------
# Newer base images bring Debian's security fixes along with pley's changes.
case $(setting PLEY_PULL_POLICY build) in
    build)
        say "Building pley (the first time takes several minutes)..."
        PLEY_COMMIT=$(git rev-parse --short=9 HEAD 2>/dev/null || echo unknown) docker compose build --pull pley
        ;;
    never) ;;
    *) docker compose pull pley ;;
esac
docker compose up -d

cid=$(docker compose ps -q pley)
[ -n "$cid" ] || die "the pley container didn't start: docker compose logs pley"
tries=0
while :; do
    state=$(docker inspect --format '{{.State.Status}} {{.RestartCount}} {{if .State.Health}}{{.State.Health.Status}}{{end}}' "$cid")
    # Restarts while it starts mean pley stops right away.
    case $state in
        "running "*" healthy") break ;;
        "running 0 "*) ;;
        *) tries=90 ;;
    esac
    tries=$((tries + 1))
    if [ "$tries" -gt 90 ]; then
        docker logs --tail 30 "$cid" >&2 || true
        die "pley didn't start (container: $state). Its log is above; the folders' owners are a common cause."
    fi
    sleep 2
done

# ---- how to reach it -------------------------------------------------------
addr=$(docker compose port pley 7539 2>/dev/null | head -n 1 || true)
case $addr in
    0.0.0.0:* | "[::]:"*) addr="localhost:${addr##*:}" ;;
esac
started=$(docker inspect --format '{{.State.StartedAt}}' "$cid")
code=$(docker logs --since "$started" "$cid" 2>&1 |
    sed -n 's/.*setup[?#]code=\([A-Z0-9-]*\).*/\1/p' | tail -n 1)

say ""
if [ -n "$code" ]; then
    say "pley is running. Create the administrator account with setup code $code"
    say "(a new code is made each time pley starts until then):"
    [ -z "$addr" ] || say "  http://$addr/setup?code=$code"
    say "  https://<your domain>/setup?code=$code   through your reverse proxy"
    say ""
    say "Then add libraries from folders under /media, such as /media/Movies."
else
    say "pley is running${addr:+ at http://$addr}."
fi
if [ -n "$addr" ]; then
    say "Point your reverse proxy at http://$addr (README.md has Caddy, nginx and Traefik examples)."
else
    say "Your reverse proxy reaches it at http://pley:7539 on its Docker network."
fi
