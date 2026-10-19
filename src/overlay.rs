//! Miras flutuantes nativas. Todos os métodos desta unidade são executados
//! exclusivamente pela thread da interface (a que processa mensagens Win32).
#![cfg(windows)]

use crate::profile::{Profile, StepKind};
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, Ellipse, EndPaint,
    FillRect, PAINTSTRUCT, PS_SOLID, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
    DT_CENTER, DT_SINGLELINE, DT_VCENTER,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowRect, GetWindowTextW,
    RegisterClassW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow,
    WNDCLASSW, HTCAPTION, LWA_ALPHA, LWA_COLORKEY, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WM_NCHITTEST, WM_PAINT,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const SIZE: i32 = 48;
const RADIUS: i32 = SIZE / 2;
// COLORREF = 0x00BBGGRR. Esta cor é recortada, inclusive nos cantos quadrados.
const KEY: u32 = 0x00FF00FF;
const BLUE: u32 = 0x00D98A16;
const GREEN: u32 = 0x0078C348;
const RED: u32 = 0x005151E5;
const WHITE: u32 = 0x00FFFFFF;
const OUTLINE: u32 = 0x00271A12;
const CLASS: &str = "AutoClickPro.NativeTargetOverlay.V2";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    match msg {
        // Arrastar uma mira usa o movimento nativo de janela do Windows,
        // sem prender o mouse nem implementar hooks globais.
        WM_NCHITTEST => HTCAPTION as isize,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_null() {
                let mut rect = RECT { left: 0, top: 0, right: SIZE, bottom: SIZE };
                let clear = CreateSolidBrush(KEY);
                FillRect(hdc, &rect, clear);
                DeleteObject(clear);

                let mut title = [0u16; 32];
                let len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32).max(0) as usize;
                let is_start = len > 0 && title[len - 1] == b'S' as u16;
                let is_end = len > 0 && title[len - 1] == b'E' as u16;
                let color = if is_start { GREEN } else if is_end { RED } else { BLUE };

                let fill = CreateSolidBrush(color);
                let pen = CreatePen(PS_SOLID, 2, OUTLINE);
                let old_fill = SelectObject(hdc, fill);
                let old_pen = SelectObject(hdc, pen);
                Ellipse(hdc, 2, 2, SIZE - 2, SIZE - 2);
                SelectObject(hdc, old_fill);
                SelectObject(hdc, old_pen);
                DeleteObject(fill);
                DeleteObject(pen);

                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, WHITE);
                rect.top = 0;
                rect.bottom = SIZE;
                DrawTextW(hdc, title.as_ptr(), len as i32, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }
            EndPaint(hwnd, &ps);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Endpoint { Start, End }

struct Marker {
    hwnd: HWND,
    index: usize,
    endpoint: Endpoint,
    center: (i32, i32),
}

impl Marker {
    fn create(index: usize, endpoint: Endpoint, center: (i32, i32), is_drag: bool) -> Result<Self, String> {
        let label = match endpoint {
            Endpoint::Start if is_drag => format!("{}S", index + 1),
            Endpoint::Start => format!("{}", index + 1),
            Endpoint::End => format!("{}E", index + 1),
        };
        let classname = wide(CLASS);
        let title = wide(&label);
        // SAFETY: todas as chamadas são feitas na thread de UI, a classe foi
        // registrada antes e as strings UTF-16 permanecem vivas durante a chamada.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                classname.as_ptr(), title.as_ptr(), WS_POPUP,
                center.0.saturating_sub(RADIUS), center.1.saturating_sub(RADIUS), SIZE, SIZE,
                null_mut(), null_mut(), GetModuleHandleW(null()), null(),
            )
        };
        if hwnd.is_null() { return Err("Falha ao criar janela flutuante de mira.".into()); }
        if unsafe { SetLayeredWindowAttributes(hwnd, KEY, 235, LWA_COLORKEY | LWA_ALPHA) } == 0 {
            unsafe { DestroyWindow(hwnd); }
            return Err("Falha ao habilitar transparência da mira.".into());
        }
        unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE); }
        Ok(Self { hwnd, index, endpoint, center })
    }

    fn move_to(&mut self, center: (i32, i32)) {
        if center == self.center { return; }
        unsafe {
            SetWindowPos(
                self.hwnd, null_mut(), center.0.saturating_sub(RADIUS), center.1.saturating_sub(RADIUS),
                0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        self.center = center;
    }

    fn actual_center(&self) -> Option<(i32, i32)> {
        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if unsafe { GetWindowRect(self.hwnd, &mut rect) } == 0 { None }
        else { Some((rect.left.saturating_add(RADIUS), rect.top.saturating_add(RADIUS))) }
    }
}

impl Drop for Marker {
    fn drop(&mut self) { unsafe { DestroyWindow(self.hwnd); } }
}

pub struct OverlayManager {
    markers: Vec<Marker>,
    visible: bool,
}

impl OverlayManager {
    pub fn new() -> Result<Self, String> {
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: unsafe { GetModuleHandleW(null()) },
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        if unsafe { RegisterClassW(&wc) } == 0 {
            return Err("Não foi possível registrar a classe de miras do Windows.".into());
        }
        Ok(Self { markers: Vec::new(), visible: true })
    }

    pub fn is_visible(&self) -> bool { self.visible }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        for marker in &self.markers {
            unsafe { ShowWindow(marker.hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE }); }
        }
    }

    pub fn rebuild(&mut self, profile: &Profile) -> Result<(), String> {
        // Drop destrói as janelas anteriores e evita vazamento de HWNDs.
        self.markers.clear();
        for (index, step) in profile.steps.iter().enumerate() {
            self.markers.push(Marker::create(index, Endpoint::Start, (step.x, step.y), step.kind == StepKind::Drag)?);
            if step.kind == StepKind::Drag {
                self.markers.push(Marker::create(index, Endpoint::End, (step.x2, step.y2), true)?);
            }
        }
        if !self.visible { self.set_visible(false); }
        Ok(())
    }

    pub fn position_changed_by_editor(&mut self, profile: &Profile) {
        for marker in &mut self.markers {
            if let Some(step) = profile.steps.get(marker.index) {
                let desired = if marker.endpoint == Endpoint::End { (step.x2, step.y2) } else { (step.x, step.y) };
                marker.move_to(desired);
            }
        }
    }

    // Retorna true se houve movimento. Chamado pelo timer Slint somente quando
    // a automação estiver parada; a gravação final é adiada pelo chamador.
    pub fn pull_positions(&mut self, profile: &mut Profile) -> bool {
        if !self.visible { return false; }
        let mut changed = false;
        for marker in &mut self.markers {
            if let Some(actual) = marker.actual_center() {
                if actual == marker.center { continue; }
                marker.center = actual;
                if let Some(step) = profile.steps.get_mut(marker.index) {
                    match marker.endpoint {
                        Endpoint::Start => { step.x = actual.0; step.y = actual.1; }
                        Endpoint::End => { step.x2 = actual.0; step.y2 = actual.1; }
                    }
                    changed = true;
                }
            }
        }
        changed
    }
}
