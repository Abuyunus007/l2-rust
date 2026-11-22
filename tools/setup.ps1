# L2 Rust server setup for Windows: checks the tools, builds the programs, then runs l2-setup,
# which makes the server folder and sets up the database (new, or an existing L2JMobius H5
# database). Start it with setup.cmd in the repository; any arguments go on to l2-setup
# (l2-setup --help lists them; relative paths are relative to the folder you start it from).
# Nothing is installed without asking first.

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

function Ask-YesNo([string]$question) {
    $answer = Read-Host "$question (y/n)"
    return $answer -match '^(y|yes|j|ja)$'
}

function Has-Command([string]$name) {
    return [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

Write-Host '== L2 Rust server: build and setup ==' -ForegroundColor Cyan

# 1. Rust (rustup installs the toolchain of rust-toolchain.toml by itself).
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (-not (Has-Command 'cargo') -and (Test-Path (Join-Path $cargoBin 'cargo.exe'))) {
    $env:Path = "$cargoBin;$env:Path"
}
if (-not (Has-Command 'cargo')) {
    Write-Host 'Rust is not installed.'
    if ((Has-Command 'winget') -and (Ask-YesNo 'Install Rust (rustup) with winget now?')) {
        winget install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements
        $env:Path = "$cargoBin;$env:Path"
    }
    if (-not (Has-Command 'cargo')) {
        Write-Host 'Install Rust from https://rustup.rs and run setup.cmd again.' -ForegroundColor Yellow
        exit 1
    }
}

# 2. The C++ build tools Rust needs on Windows (the linker).
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$hasVc = $false
if (Test-Path $vswhere) {
    $vc = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $hasVc = [bool]$vc
}
if (-not $hasVc) {
    Write-Host 'The Visual Studio C++ build tools (needed by Rust on Windows) were not found.'
    if ((Has-Command 'winget') -and (Ask-YesNo 'Install them with winget now (several GB, takes a while)?')) {
        winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --accept-source-agreements --override '--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    } else {
        Write-Host 'Install "Desktop development with C++" from https://visualstudio.microsoft.com/visual-cpp-build-tools/ if the build fails with "link.exe not found".' -ForegroundColor Yellow
    }
}

# 3. MariaDB or MySQL (only checked; the setup asks for the connection later).
$db = Get-Service -ErrorAction SilentlyContinue | Where-Object { $_.Name -match 'maria|mysql' }
if (-not $db) {
    Write-Host 'No MariaDB or MySQL service was found on this PC (a database on another machine works too).'
    if ((Has-Command 'winget') -and (Ask-YesNo 'Install MariaDB with winget now?')) {
        Write-Host 'The MariaDB installer window opens: set a root password, keep "Install as service" ticked, and remember the password: the setup asks for it.' -ForegroundColor Yellow
        winget install --id MariaDB.Server -e --interactive --accept-package-agreements --accept-source-agreements
    }
}

# 4. Build (from the repository, wherever this was started from).
Write-Host "`nBuilding (the first build takes several minutes) ..." -ForegroundColor Cyan
cargo build --release --manifest-path (Join-Path $repo 'Cargo.toml') -p login-server -p game-server -p l2-supervisor -p l2-admin-gui -p l2-db-installer -p l2-setup
if ($LASTEXITCODE -ne 0) {
    Write-Host 'The build failed (see above).' -ForegroundColor Red
    exit 1
}

# 5. The server folder and the database (relative paths: from the folder this was started in).
& (Join-Path $repo 'target\release\l2-setup.exe') --repo $repo @args
exit $LASTEXITCODE
