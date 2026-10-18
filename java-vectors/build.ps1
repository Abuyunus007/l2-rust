# Builds the Java vector generators against the L2JMobius sources and runs them.
#
#   .\java-vectors\build.ps1                 # build, then run "all" generators into java-vectors\out
#   .\java-vectors\build.ps1 crypto          # build, then run one generator
#   .\java-vectors\build.ps1 -NoRun          # build only
#
# Environment:
#   L2J_SRC   Java source root of L2JMobius (the folder that contains "org\"). Default: C:\l2-rust\l2jmobius\java
#   L2J_LIBS  Folder with the third-party jars.                              Default: <L2J_SRC>\..\dist\libs

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
& javac -encoding UTF-8 -nowarn -implicit:class -d $build -sourcepath $src -cp $cp $files
if ($LASTEXITCODE -ne 0) { throw "javac failed" }

if ($NoRun) { return }

# Same-file-system, no clock, no randomness: running this twice gives byte-identical files.
& java -cp "$build;$cp" l2rust.vectors.Main $Generator $out
if ($LASTEXITCODE -ne 0) { throw "vector generation failed" }
