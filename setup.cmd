@echo off
rem L2 Rust server: build and set up (see tools\setup.ps1). Arguments go on to l2-setup.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\setup.ps1" %*
rem Keep the window open: the result (or the error) stays readable after a double-click.
pause
