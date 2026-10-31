@echo off
rem Starts the database installer (GUI, or console with --console).
cd /d "%~dp0"
if exist "l2-db-installer.exe" (
  "l2-db-installer.exe" --sql "%~dp0sql" %*
  goto :eof
)
if exist "..\..\target\release\l2-db-installer.exe" (
  "..\..\target\release\l2-db-installer.exe" --sql "%~dp0sql" %*
  goto :eof
)
echo l2-db-installer.exe not found. Build it: cargo build --release -p l2-db-installer
pause
