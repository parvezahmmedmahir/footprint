@echo off
setlocal

echo ===================================================
echo     Lux Chart - Environment Setup & Launcher
echo ===================================================

echo.
echo [1/3] Checking for Visual Studio Build Tools Environment...

REM Defines common paths for VsDevCmd.bat - Added support for VS 2022/2026
set "VS2026_PATH=C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat"
set "VS2022_ENT=C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat"
set "VS2022_PRO=C:\Program Files\Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat"
set "VS2022_COM=C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat"
set "VS2022_BT=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"

if exist "%VS2026_PATH%" (
    echo Found VS 2026 Build Tools. Initializing...
    call "%VS2026_PATH%"
    goto :build_step
)

if exist "%VS2022_BT%" (
    echo Found VS 2022 Build Tools. Initializing...
    call "%VS2022_BT%"
    goto :build_step
)

if exist "%VS2022_ENT%" (
    echo Found VS 2022 Enterprise. Initializing...
    call "%VS2022_ENT%"
    goto :build_step
)

if exist "%VS2022_PRO%" (
    echo Found VS 2022 Professional. Initializing...
    call "%VS2022_PRO%"
    goto :build_step
)

if exist "%VS2022_COM%" (
    echo Found VS 2022 Community. Initializing...
    call "%VS2022_COM%"
    goto :build_step
)

echo.
echo WARNING: Could not find standard location for VsDevCmd.bat
echo Attempting to check if tools are already in PATH...
where link >nul 2>nul
if %errorlevel% neq 0 (
    echo ERROR: Linker 'link.exe' not found.
    echo Please make sure you have installed "Desktop development with C++" workload.
    pause
    exit /b 1
)

:build_step
echo.
echo [2/3] Verifying Rust (Cargo)...
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo ERROR: Rust is not installed or not in PATH.
    pause
    exit /b 1
)
echo Rust found.

echo.
echo [3/3] Building and Running Lux Chart...
echo.

REM Set high priority for better compilation performance
start /b /high cargo run --release

endlocal
