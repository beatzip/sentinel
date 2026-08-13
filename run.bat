@echo off
REM ============================================================
REM  Sentinel AI - simple launcher for Windows
REM  Usage:
REM    run.bat analyze match.dem        (анализ, с учётом памяти)
REM    run.bat learn   match.dem        (анализ + обучение памяти)
REM    run.bat memory                  (показать, чему научилось)
REM    run.bat memory reset            (очистить память)
REM    run.bat build                   (только собрать)
REM ============================================================
setlocal

REM Build once if the binary is missing.
if not exist "target\release\sentinel.exe" (
  echo Building Sentinel AI ^(first run, release^)...
  cargo build --release
  if errorlevel 1 (
    echo Build failed.
    exit /b 1
  )
)

if "%1"=="" (
  echo Usage: run.bat ^<analyze^|learn^|memory^|build^> [args]
  echo.
  echo   run.bat analyze match.dem     Analyze a demo ^(uses memory if present^)
  echo   run.bat learn   match.dem     Analyze and train memory
  echo   run.bat memory               Show what Sentinel learned
  echo   run.bat memory reset         Clear memory
  echo   run.bat build                Rebuild the binary
  exit /b 0
)

if "%1"=="build" (
  cargo build --release
  exit /b %errorlevel%
)

target\release\sentinel.exe %*
