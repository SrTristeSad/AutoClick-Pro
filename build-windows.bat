@echo off
setlocal
cd /d "%~dp0"
where cargo >nul 2>nul
if errorlevel 1 (
  echo ERRO: Rust / Cargo nao encontrado.
  echo Instale Rust pelo site https://rustup.rs/
  pause
  exit /b 1
)
echo [1/3] Verificando testes do projeto...
cargo test
if errorlevel 1 (
  echo.
  echo ERRO: Testes falharam. O executavel nao sera gerado.
  pause
  exit /b 1
)
echo [2/3] Compilando versao Release... 
cargo build --release
if errorlevel 1 (
  echo.
  echo ERRO: A compilacao falhou. Confira as mensagens acima.
  pause
  exit /b 1
)
echo [3/3] Build concluido.
echo Arquivo: "%~dp0target\release\autoclick-pro-native.exe"
pause
