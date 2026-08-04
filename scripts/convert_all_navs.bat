@echo off
title Nav Mesh Converter
cd /d "%~dp0"

echo ========================================
echo   Nav Mesh to JSON Converter
echo ========================================
echo.

echo Checking Python...
python --version >nul 2>&1
if errorlevel 1 (
    echo Error: Python not found
    echo Please install Python and awpy
    pause
    exit /b 1
)

echo.
echo Installing/updating awpy...
pip install -q awpy

echo.
echo Converting .nav files to JSON...
echo.

python convert_navs.py

echo.
echo ========================================
echo   Conversion complete!
echo ========================================
pause