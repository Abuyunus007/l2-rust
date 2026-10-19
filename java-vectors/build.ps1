# Builds the Java vector generators against the L2JMobius sources and runs them.
#
#   .\java-vectors\build.ps1                 # build, then run "all" generators into java-vectors\out
#   .\java-vectors\build.ps1 crypto          # build, then run one generator
#   .\java-vectors\build.ps1 -NoRun          # build only
#
# Environment:
#   L2J_SRC   Java source root of L2JMobius (the folder that contains "org\"). Default: C:\l2-rust\l2jmobius\java
#   L2J_LIBS  Folder with the third-party jars.                              Default: <L2J_SRC>\..\dist\libs
#   L2J_GAME_ROOT  Game server root (has config\ and data\).                 Default: <L2J_SRC>\..\dist\game   (stats, formulas only)

param(
    [string]$Generator = "all",
    [switch]$NoRun
)

$ErrorActionPreference = "Stop"

$here = $PSScriptRoot
$src = if ($env:L2J_SRC) { $env:L2J_SRC } else { "C:\l2-rust\l2jmobius\java" }
$libs = if ($env:L2J_LIBS) { $env:L2J_LIBS } else { Join-Path (Split-Path $src -Parent) "dist\libs" }
$build = Join-Path $here "build"
$out = Join-Path $here "out"

if (-not (Test-Path (Join-Path $src "org"))) { throw "L2J_SRC '$src' does not contain org/. Set the L2J_SRC environment variable." }

# Always compile from the sources (never use l2jmobius\bin, it may be stale).
if (Test-Path $build) { Remove-Item -Recurse -Force $build }
New-Item -ItemType Directory -Force $build | Out-Null

$files = Get-ChildItem -Path (Join-Path $here "src") -Recurse -Filter *.java | ForEach-Object { $_.FullName }
$cp = Join-Path $libs "*"

# Stat and formula generators (M4-JV-01): the effect classes are compiled from the datapack scripts (second source path root), and the classes the server loads by name
# (Func*, NPC instance types, item types) are not found by javac's dependency scan, so they are compiled explicitly.
$gameRoot = if ($env:L2J_GAME_ROOT) { $env:L2J_GAME_ROOT } else { Join-Path (Split-Path $src -Parent) "dist\game" }
$scripts = Join-Path $gameRoot "data\scripts"
$sourcepath = if (Test-Path $scripts) { "$src;$scripts" } else { $src }
foreach ($d in @("entity\actor\instance", "mechanics\stats\functions", "entity\item")) {
    $dir = Join-Path $src "org\l2jmobius\gameserver\$d"
    if (Test-Path $dir) { $files += Get-ChildItem -Path $dir -Filter *.java | ForEach-Object { $_.FullName } }
}

& javac -encoding UTF-8 -nowarn -implicit:class -d $build -sourcepath $sourcepath -cp $cp $files
if ($LASTEXITCODE -ne 0) { throw "javac failed" }

if ($NoRun) { return }

# The stat and formula generators read ./config and ./data of the game server root, so they run with that as the working directory (the output path is absolute).
if (($Generator -eq "stats") -or ($Generator -eq "formulas")) {
    if (-not (Test-Path (Join-Path $gameRoot "config\Server.ini"))) { throw "L2J_GAME_ROOT '$gameRoot' has no config\Server.ini" }
    Push-Location $gameRoot
    try {
        & java -Xmx2g -cp "$build;$cp" l2rust.vectors.Main $Generator $out
        if ($LASTEXITCODE -ne 0) { throw "vector generation failed" }
    }
    finally { Pop-Location }
    return
}

# Same-file-system, no clock, no randomness: running this twice gives byte-identical files.
& java -cp "$build;$cp" l2rust.vectors.Main $Generator $out
if ($LASTEXITCODE -ne 0) { throw "vector generation failed" }
