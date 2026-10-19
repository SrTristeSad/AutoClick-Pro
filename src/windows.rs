#![cfg(windows)]

use std::mem::size_of;
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

pub fn cursor_position() -> Result<(i32, i32), String> {
    let mut point = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err("Não foi possível obter a posição do mouse.".into());
    }
    Ok((point.x, point.y))
}

pub fn move_cursor(x: i32, y: i32) -> Result<(), String> {
    if unsafe { SetCursorPos(x, y) } == 0 {
        return Err("Não foi possível mover o cursor.".into());
    }
    Ok(())
}

fn mouse_event(flags: u32) -> Result<(), String> {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    };
    let sent = unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) };
    if sent == 1 { Ok(()) } else { Err("O Windows bloqueou o envio do clique (SendInput).".into()) }
}

pub fn down() -> Result<(), String> { mouse_event(MOUSEEVENTF_LEFTDOWN) }
pub fn up() -> Result<(), String> { mouse_event(MOUSEEVENTF_LEFTUP) }

pub fn click(x: i32, y: i32) -> Result<(), String> {
    move_cursor(x, y)?;
    down()?;
    up()
}

pub fn key_down(vkey: i32) -> bool {
    (unsafe { GetAsyncKeyState(vkey) } as u16 & 0x8000) != 0
}


// Seletor de arquivos nativo do Windows, evitando dependências de Electron,
// navegadores embutidos ou bibliotecas gráficas adicionais.
pub fn choose_json_file(save: bool) -> Result<Option<std::path::PathBuf>, String> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW,
        OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT,
        OFN_PATHMUSTEXIST,
    };

    let filter: Vec<u16> = "Arquivos JSON (*.json)\0*.json\0Todos os arquivos (*.*)\0*.*\0\0"
        .encode_utf16().collect();
    let extension: Vec<u16> = "json\0".encode_utf16().collect();
    let title: Vec<u16> = (if save { "Exportar perfil\0" } else { "Importar perfil\0" })
        .encode_utf16().collect();
    let mut filename = [0u16; 32768];
    if save {
        let name: Vec<u16> = "AutoClick-Pro.json".encode_utf16().collect();
        filename[..name.len()].copy_from_slice(&name);
    }
    let mut dialog: OPENFILENAMEW = unsafe { zeroed() };
    dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.nFilterIndex = 1;
    dialog.lpstrFile = filename.as_mut_ptr();
    dialog.nMaxFile = filename.len() as u32;
    dialog.lpstrDefExt = extension.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.Flags = OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST |
        (if save { OFN_OVERWRITEPROMPT } else { OFN_FILEMUSTEXIST });

    let ok = unsafe {
        if save { GetSaveFileNameW(&mut dialog) } else { GetOpenFileNameW(&mut dialog) }
    };
    if ok == 0 {
        let code = unsafe { CommDlgExtendedError() };
        if code == 0 { return Ok(None); } // Cancelado pelo usuário.
        return Err(format!("Não foi possível abrir a seleção de arquivos (erro {code:#x})."));
    }
    let end = filename.iter().position(|ch| *ch == 0).unwrap_or(filename.len());
    Ok(Some(std::path::PathBuf::from(
        String::from_utf16(&filename[..end]).map_err(|_| "Nome de arquivo inválido.".to_string())?
    )))
}
