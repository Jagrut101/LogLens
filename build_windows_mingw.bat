@echo off
setlocal
where cargo >nul 2>nul || (echo ERROR: cargo not found & exit /b 1)
where g++ >nul 2>nul || (echo ERROR: MinGW g++ not found in PATH & exit /b 1)

rustup target add x86_64-pc-windows-gnu
if errorlevel 1 exit /b 1

echo Building LogLens optimized Windows release...
set CARGO_PROFILE_RELEASE_DEBUG=false
cargo build --locked --release --target x86_64-pc-windows-gnu
if errorlevel 1 exit /b 1

if not exist dist mkdir dist
copy /Y target\x86_64-pc-windows-gnu\release\loglens-replica.exe dist\LogLens.exe >nul
echo.
echo READY: dist\LogLens.exe
echo IMPORTANT: benchmark this Release executable, not cargo run/debug.
endlocal
