#!/usr/bin/env sh
# Builds the Java vector generators against the L2JMobius sources and runs them.
#
#   ./java-vectors/build.sh              # build, then run "all" generators into java-vectors/out
#   ./java-vectors/build.sh crypto       # build, then run one generator
#   ./java-vectors/build.sh stats        # stat engine vectors (runs from the game server root)
#   ./java-vectors/build.sh formulas     # Formulas vectors (runs from the game server root)
#   NO_RUN=1 ./java-vectors/build.sh     # build only
#
# Environment:
#   L2J_SRC        Java source root of L2JMobius (the folder that contains "org/"). Required unless the default exists.
#   L2J_LIBS       Folder with the third-party jars. Default: <L2J_SRC>/../dist/libs
#   L2J_GAME_ROOT  Game server root (has config/ and data/). Default: <L2J_SRC>/../dist/game   (stats, formulas only)
set -eu

# Windows (Git Bash / MSYS) needs ';' as classpath separator and Windows-style paths for java/javac.
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) sep=';'; here=$(cd "$(dirname "$0")" && pwd -W); default_src='C:/l2-rust/l2jmobius/java' ;;
  *) sep=':'; here=$(cd "$(dirname "$0")" && pwd); default_src=/opt/l2jmobius/java ;;
esac

src=${L2J_SRC:-$default_src}
libs=${L2J_LIBS:-$src/../dist/libs}
game_root=${L2J_GAME_ROOT:-$src/../dist/game}
build=$here/build
out=$here/out
generator=${1:-all}

[ -d "$src/org" ] || { echo "L2J_SRC '$src' does not contain org/. Set the L2J_SRC environment variable." >&2; exit 1; }

# Always compile from the sources (never use l2jmobius/bin, it may be stale).
rm -rf "$build"
mkdir -p "$build"

find "$here/src" -name '*.java' > "$build/sources.txt"

# Stat and formula generators (M4-JV-01): the effect classes are compiled from the datapack scripts (second source path root), and the classes the server loads by name
# (Func*, NPC instance types, item types) are not found by javac's dependency scan, so they are compiled explicitly.
sourcepath=$src
[ -d "$game_root/data/scripts" ] && sourcepath=$src$sep$game_root/data/scripts
for d in entity/actor/instance mechanics/stats/functions entity/item; do
  [ -d "$src/org/l2jmobius/gameserver/$d" ] && find "$src/org/l2jmobius/gameserver/$d" -maxdepth 1 -name '*.java' >> "$build/sources.txt"
done

javac -encoding UTF-8 -nowarn -implicit:class -d "$build" -sourcepath "$sourcepath" -cp "$libs/*" @"$build/sources.txt"
rm -f "$build/sources.txt"

[ -n "${NO_RUN:-}" ] && exit 0

# The skill formula generator (M5-JV-03) also reads the game server root (working directory, absolute output path).
case "$generator" in
  skill-formulas)
    [ -f "$game_root/config/Server.ini" ] || { echo "L2J_GAME_ROOT '$game_root' has no config/Server.ini" >&2; exit 1; }
    (cd "$game_root" && java -Xmx2g -cp "$build$sep$libs/*" l2rust.vectors.Main "$generator" "$out")
    exit 0
    ;;
esac

# The stat and formula generators read ./config and ./data of the game server root, so they run with that as the working directory (the output path is absolute).
case "$generator" in
  stats|formulas)
    [ -f "$game_root/config/Server.ini" ] || { echo "L2J_GAME_ROOT '$game_root' has no config/Server.ini" >&2; exit 1; }
    (cd "$game_root" && java -Xmx2g -cp "$build$sep$libs/*" l2rust.vectors.Main "$generator" "$out")
    exit 0
    ;;
esac

# No clock, no randomness: running this twice gives byte-identical files.
java -cp "$build$sep$libs/*" l2rust.vectors.Main "$generator" "$out"
