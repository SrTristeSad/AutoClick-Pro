# Notas técnicas da versão nativa 0.3

- Windows 10/11 apenas; ainda sem build/teste real neste ambiente.
- Macros de clique, duplo clique e arrasto, alvos arrastáveis, perfis JSON e atalhos globais.
- Captura/gravação contínua de macros e reconhecimento de imagens não implementados.
- Uso do mouse real pode interferir em janelas durante a execução.
- Testar DPI, múltiplos monitores, comportamento ao fechar e janelas executadas como administrador.
- Compilação: execute `build-windows.bat`, que roda `cargo test` e `cargo build --release`.
- Código Python histórico: branch `python-legacy`; projeto original conserva `icone.ico`.
