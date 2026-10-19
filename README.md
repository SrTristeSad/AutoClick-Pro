# AutoClick-Pro — Rust + Slint

Aplicativo nativo de automação para **Windows 10/11**, escrito em Rust com interface Slint.

> Estado: código-fonte v0.3, ainda **não compilado/testado em Windows** neste ambiente. Rode os testes e valide antes de distribuir.

## Funcionalidades

- Cliques simples, duplos, e arrastar mouse (Windows API).
- Miras flutuantes arrastáveis sobre o desktop (clique azul, início verde, fim vermelho).
- Editor de sequência: coordenadas, tempo, ordem e repetições.
- Importar/exportar JSON, incluindo conversão de macros do projeto Python anterior.
- Perfil padrão em `%APPDATA%\AutoClickPro\profile.json`.
- Atalhos F5 (iniciar/parar), F6 (mostrar/esconder miras), F8/F9 (capturar posições), Escape (interromper).

## Compilação

Instale [Rust](https://rustup.rs) e Visual Studio Build Tools com MSVC C++ e Windows SDK. Execute `build-windows.bat`; após compilar, o executável estará em `target\release\autoclick-pro-native.exe`.

## Versão Python preservada

A versão Python foi preservada como instantâneo de código na branch [`python-legacy`](https://github.com/SrTristeSad/AutoClick-Pro/tree/python-legacy) e no commit [`7a1037b`](https://github.com/SrTristeSad/AutoClick-Pro/commit/7a1037b1b8d737b49f941349559979e11cf09d76).

Esse instantâneo contém os arquivos de código, documentação e dependências, **exceto o arquivo binário `icone.ico`**, que permanece disponível no [repositório de origem](https://github.com/01001110-01100101-01110010-01100100/AutoClick-Pro). O histórico completo de commits do repositório de origem continua naquele repositório; aqui foi preservada uma versão do código anterior à migração.

## Licenças

A documentação original declara CC BY-NC-SA 4.0. Preserve as atribuições e cumpra as restrições não comerciais pertinentes a qualquer material derivado. A biblioteca Slint possui seus próprios termos de licença.
