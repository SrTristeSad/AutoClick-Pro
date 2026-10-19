@echo off
setlocal EnableExtensions
cd /d "%~dp0"
title AutoClick-Pro - Compilacao Windows

echo === AutoClick-Pro Rust + Slint v0.3.1 ===
echo.
where cargo >nul 2>nul
if errorlevel 1 (
  echo [ERRO] Cargo nao encontrado. Instale Rust pelo site https://rustup.rs/
  echo Escolha o toolchain stable-x86_64-pc-windows-msvc.
  goto :erro
)
where rustc >nul 2>nul
if errorlevel 1 (
  echo [ERRO] Compilador Rust nao encontrado. Reinstale o toolchain MSVC.
  goto :erro
)
echo Ferramentas encontradas:
rustc --version
cargo --version
echo.
echo Requisitos: Visual Studio Build Tools C++ e Windows SDK instalados.
echo Se ocorrer erro link.exe, instale Desktop development with C++.
echo.
echo [1/3] Verificando manifesto Cargo...
cargo metadata --no-deps --format-version 1 >nul
if errorlevel 1 goto :erro
echo [2/3] Executando os testes Rust...
cargo test
if errorlevel 1 goto :erro
echo [3/3] Compilando executavel otimizado...
cargo build --release
if errorlevel 1 goto :erro
if not exist "target\release\autoclick-pro-native.exe" (
  echo [ERRO] O Cargo terminou sem gerar o executavel esperado.
  goto :erro
)
echo.
echo [OK] Executavel criado:
echo %CD%\target\release\autoclick-pro-native.exe
echo.
echo Faca um teste manual: clique, arrasto, F5, F6 e ESC.
pause
exit /b 0
:erro
echo.
echo [FALHA] A compilacao/teste nao foi concluida.
echo Copie as mensagens de erro para investigacao.
pause
exit /b 1
