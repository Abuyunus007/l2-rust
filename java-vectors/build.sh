#!/usr/bin/env sh
# Builds the Java vector generators against the L2JMobius sources and runs them.
#
#   ./java-vectors/build.sh              # build, then run "all" generators into java-vectors/out
#   ./java-vectors/build.sh crypto       # build, then run one generator
#   NO_RUN=1 ./java-vectors/build.sh     # build only
#
# Environment:
#   L2J_SRC   Java source root of L2JMobius (the folder that contains "org/"). Required unless the default exists.
#   L2J_LIBS  Folder with the third-party jars. Default: <L2J_SRC>/../dist/libs
set -eu

# Windows (Git Bash / MSYS) needs ';' as classpath separator and Windows-style paths for java/javac.
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) sep=';'; here=$(cd "$(dirname "$0")" && pwd -W); default_src='C:/l2-rust/l2jmobius/java' ;;
  *) sep=':'; here=$(cd "$(dirname "$0")" && pwd); default_src=/opt/l2jmobius/java ;;
esac

src=${L2J_SRC:-$default_src}
libs=${L2J_LIBS:-$src/../dist/libs}
build=$here/build
out=$here/out
generator=${1:-all}

[ -d "$src/org" ] || { echo "L2J_SRC '$src' does not contain org/. Set the L2J_SRC environment variable." >&2; exit 1; }

# Always compile from the sources (never use l2jmobius/bin, it may be stale).
rm -rf "$build"
mkdir -p "$build"

find "$here/src" -name '*.java' > "$build/sources.txt"
javac -encoding UTF-8 -nowarn -implicit:class -d "$build" -sourcepath "$src" -cp "$libs/*" @"$build/sources.txt"
rm -f "$build/sources.txt"

[ -n "${NO_RUN:-}" ] && exit 0

# No clock, no randomness: running this twice gives byte-identical files.
java -cp "$build$sep$libs/*" l2rust.vectors.Main "$generator" "$out"
