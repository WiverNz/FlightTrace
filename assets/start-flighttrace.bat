@echo off
REM One-click launcher for the packaged FlightTrace Windows archive.
REM Runs the binary from the same folder as this .bat so `web-dist` and
REM `./data/` land next to it, and keeps the terminal window open with
REM `pause` so double-clickers can see the printed URL / any error.
cd /d "%~dp0"
flighttrace.exe
pause
