#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(not(windows), allow(unused))]

#[cfg(not(windows))]
compile_error!("O motor desta primeira versão foi desenvolvido para Windows 10/11.");

mod automation;
mod overlay;
mod profile;
mod windows;

use automation::Controller;
use overlay::OverlayManager;
use profile::{Profile, Step, StepKind};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::path::Path;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

slint::include_modules!();

type State = Arc<Mutex<Profile>>;
type OverlayState = Rc<RefCell<OverlayManager>>;

fn set_status(ui: &AppWindow, text: impl Into<SharedString>) { ui.set_status(text.into()); }

fn refresh_list(ui: &AppWindow, state: &State) {
    let p = state.lock().unwrap();
    let rows: Vec<ActionRow> = p.steps.iter().enumerate().map(|(i, step)| {
        let location = match step.kind {
            StepKind::Drag => format!("({}, {}) → ({}, {})", step.x, step.y, step.x2, step.y2),
            _ => format!("({}, {})", step.x, step.y),
        };
        ActionRow {
            number: (i + 1).to_string().into(),
            action: step.kind.label().into(),
            location: location.into(),
            waiting: format!("{} ms", step.delay_ms).into(),
        }
    }).collect();
    ui.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
}

fn select_step(ui: &AppWindow, state: &State, requested: i32) {
    let profile = state.lock().unwrap();
    if requested < 0 || requested as usize >= profile.steps.len() {
        ui.set_selected_index(-1);
        return;
    }
    let step = &profile.steps[requested as usize];
    ui.set_selected_index(requested);
    ui.set_x_text(step.x.to_string().into());
    ui.set_y_text(step.y.to_string().into());
    ui.set_x2_text(step.x2.to_string().into());
    ui.set_y2_text(step.y2.to_string().into());
    ui.set_delay_text(step.delay_ms.to_string().into());
    ui.set_duration_text(step.duration_ms.to_string().into());
}

fn autosave(ui: &AppWindow, state: &State, success: &str) {
    let p = state.lock().unwrap();
    match profile::save(&p) {
        Ok(_) => set_status(ui, success),
        Err(e) => set_status(ui, format!("ERRO ao salvar: {e}")),
    }
}

fn new_step(ui: &AppWindow, state: &State, control: &Controller, overlays: &OverlayState, kind: StepKind) {
    if control.is_active() { set_status(ui, "Pare a sequência para editar."); return; }
    let (cx, cy) = windows::cursor_position().unwrap_or((400, 300));
    let (x, y) = (cx.saturating_add(150), cy.saturating_add(50));
    let index = {
        let mut p = state.lock().unwrap();
        if p.steps.len() >= 200 { set_status(ui, "Limite de 200 ações."); return; }
        p.steps.push(Step::new(kind, x, y));
        p.steps.len() as i32 - 1
    };
    refresh_list(ui, state);
    select_step(ui, state, index);
    rebuild_targets(ui, state, overlays);
    autosave(ui, state, "Ação e mira adicionadas. Arraste-a até o destino.");
}

fn parse_i32(input: SharedString, label: &str) -> Result<i32, String> {
    input.trim().parse::<i32>().map_err(|_| format!("{label} precisa ser um número inteiro."))
}
fn parse_u32(input: SharedString, label: &str, min: u32, max: u32) -> Result<u32, String> {
    let value = input.trim().parse::<u32>().map_err(|_| format!("{label} precisa ser um número inteiro positivo."))?;
    if !(min..=max).contains(&value) { return Err(format!("{label}: permitido de {min} a {max}.")); }
    Ok(value)
}

fn update_selected(ui: &AppWindow, state: &State, control: &Controller, overlays: &OverlayState) -> Result<(), String> {
    if control.is_active() { return Err("Pare a sequência para editar.".into()); }
    let index = ui.get_selected_index();
    if index < 0 { return Err("Selecione uma ação.".into()); }
    let mut updated = {
        let p = state.lock().unwrap();
        p.steps.get(index as usize).ok_or("Ação não encontrada.")?.clone()
    };
    updated.x = parse_i32(ui.get_x_text(), "X")?;
    updated.y = parse_i32(ui.get_y_text(), "Y")?;
    updated.x2 = parse_i32(ui.get_x2_text(), "Destino X")?;
    updated.y2 = parse_i32(ui.get_y2_text(), "Destino Y")?;
    updated.delay_ms = parse_u32(ui.get_delay_text(), "Espera", 20, 60_000)?;
    updated.duration_ms = parse_u32(ui.get_duration_text(), "Duração", 10, 30_000)?;
    let mut p = state.lock().unwrap();
    let original = p.steps[index as usize].clone();
    p.steps[index as usize] = updated;
    if let Err(e) = p.validate() {
        p.steps[index as usize] = original;
        return Err(e);
    }
    drop(p);
    refresh_list(ui, state);
    overlays.borrow_mut().position_changed_by_editor(&state.lock().unwrap());
    autosave(ui, state, "Alterações aplicadas e miras atualizadas.");
    Ok(())
}

fn cycles_from_ui(ui: &AppWindow) -> Result<u32, String> {
    parse_u32(ui.get_cycles_text(), "Ciclos", 0, 100_000)
}

fn load_into_ui(ui: &AppWindow, state: &State, overlays: &OverlayState, p: Profile) {
    let cycles = p.cycles;
    *state.lock().unwrap() = p;
    ui.set_cycles_text(cycles.to_string().into());
    refresh_list(ui, state);
    select_step(ui, state, 0);
    rebuild_targets(ui, state, overlays);
}

fn rebuild_targets(ui: &AppWindow, state: &State, overlays: &OverlayState) {
    let result = overlays.borrow_mut().rebuild(&state.lock().unwrap());
    if let Err(error) = result { set_status(ui, format!("Erro ao criar as miras: {error}")); }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli: Vec<String> = std::env::args().skip(1).collect();
    let loaded = if cli.len() == 2 && cli[0] == "--import" {
        profile::load_file(Path::new(&cli[1]))
    } else { profile::load() };
    let (initial, error) = match loaded {
        Ok(p) => (p, None),
        Err(e) => (Profile::default(), Some(e)),
    };
    let ui = AppWindow::new()?;
    let state: State = Arc::new(Mutex::new(initial.clone()));
    let control = Controller::default();
    let overlays: OverlayState = Rc::new(RefCell::new(OverlayManager::new()
        .map_err(std::io::Error::other)?));
    let exit = Arc::new(AtomicBool::new(false));

    load_into_ui(&ui, &state, &overlays, initial);
    if let Some(e) = error.as_ref() { set_status(&ui, format!("Falha ao carregar perfil: {e}")); }
    if cli.len() == 2 && cli[0] == "--import" && error.is_none() {
        autosave(&ui, &state, "JSON importado; perfil salvo em AppData.");
    }

    {
        let weak = ui.as_weak(); let state = state.clone(); let control = control.clone(); let overlays = overlays.clone();
        ui.on_add_click(move || new_step(&weak.unwrap(), &state, &control, &overlays, StepKind::Click));
    }
    {
        let weak = ui.as_weak(); let state = state.clone(); let control = control.clone(); let overlays = overlays.clone();
        ui.on_add_double_click(move || new_step(&weak.unwrap(), &state, &control, &overlays, StepKind::DoubleClick));
    }
    {
        let weak = ui.as_weak(); let state = state.clone(); let control = control.clone(); let overlays = overlays.clone();
        ui.on_add_drag(move || new_step(&weak.unwrap(), &state, &control, &overlays, StepKind::Drag));
    }
    {
        let weak = ui.as_weak(); let state = state.clone();
        ui.on_select_step(move |idx| select_step(&weak.unwrap(), &state, idx));
    }
    {
        let weak = ui.as_weak(); let state = state.clone(); let control = control.clone(); let overlays = overlays.clone();
        ui.on_update_selected(move || {
            let ui = weak.unwrap();
            if let Err(e) = update_selected(&ui, &state, &control, &overlays) { set_status(&ui, e); }
        });
    }
    {
        let weak = ui.as_weak(); let state = state.clone(); let control = control.clone(); let overlays = overlays.clone();
        ui.on_remove_selected(move || {
            let ui = weak.unwrap();
            if control.is_active() { set_status(&ui, "Pare a sequência para editar."); return; }
            let index = ui.get_selected_index();
            if index < 0 { return; }
            let next_index = {
                let mut p = state.lock().unwrap();
                if index as usize >= p.steps.len() { return; }
                p.steps.remove(index as usize);
                (index as usize).min(p.steps.len().saturating_sub(1)) as i32
            };
            refresh_list(&ui, &state);
            rebuild_targets(&ui, &state, &overlays);
            if state.lock().unwrap().steps.is_empty() { select_step(&ui, &state, -1); }
            else { select_step(&ui, &state, next_index); }
            autosave(&ui, &state, "Ação removida. Perfil salvo.");
        });
    }
    // Reordena os passos e recria as miras com a numeração correta.
    {
        let weak = ui.as_weak(); let state = state.clone();
        let control = control.clone(); let overlays = overlays.clone();
        ui.on_move_selected(move |delta| {
            let ui = weak.unwrap();
            if control.is_active() { set_status(&ui, "Pare a sequência para reordenar."); return; }
            let selected = ui.get_selected_index();
            let swapped = {
                let mut p = state.lock().unwrap();
                let destination = selected.saturating_add(delta);
                if selected < 0 || destination < 0 ||
                    selected as usize >= p.steps.len() || destination as usize >= p.steps.len() {
                    false
                } else {
                    p.steps.swap(selected as usize, destination as usize);
                    true
                }
            };
            if swapped {
                refresh_list(&ui, &state);
                select_step(&ui, &state, selected + delta);
                rebuild_targets(&ui, &state, &overlays);
                autosave(&ui, &state, "Sequência reordenada e salva.");
            }
        });
    }
    // Importa/exporta perfis usando o diálogo nativo do Windows.
    {
        let weak = ui.as_weak(); let state = state.clone();
        let control = control.clone(); let overlays = overlays.clone();
        ui.on_import_profile(move || {
            let ui = weak.unwrap();
            if control.is_active() { set_status(&ui, "Pare a sequência para importar."); return; }
            let previously_visible = overlays.borrow().is_visible();
            overlays.borrow_mut().set_visible(false);
            let result = windows::choose_json_file(false);
            overlays.borrow_mut().set_visible(previously_visible);
            match result {
                Ok(Some(path)) => match profile::load_file(&path) {
                    Ok(p) => {
                        load_into_ui(&ui, &state, &overlays, p);
                        autosave(&ui, &state, "Perfil importado e salvo como padrão.");
                    }
                    Err(e) => set_status(&ui, format!("Erro na importação: {e}")),
                },
                Ok(None) => (),
                Err(e) => set_status(&ui, e),
            }
        });
    }
    {
        let weak = ui.as_weak(); let state = state.clone();
        let control = control.clone(); let overlays = overlays.clone();
        ui.on_export_profile(move || {
            let ui = weak.unwrap();
            if control.is_active() { set_status(&ui, "Pare a sequência para exportar."); return; }
            let cycles = match cycles_from_ui(&ui) {
                Ok(value) => value,
                Err(e) => { set_status(&ui, e); return; }
            };
            let snapshot = {
                let p = state.lock().unwrap();
                let mut snapshot = p.clone();
                snapshot.cycles = cycles;
                snapshot
            };
            let previously_visible = overlays.borrow().is_visible();
            overlays.borrow_mut().set_visible(false);
            let result = windows::choose_json_file(true);
            overlays.borrow_mut().set_visible(previously_visible);
            match result {
                Ok(Some(path)) => match profile::save_file(&snapshot, &path) {
                    Ok(()) => set_status(&ui, format!("Perfil exportado: {}", path.display())),
                    Err(e) => set_status(&ui, e),
                },
                Ok(None) => (),
                Err(e) => set_status(&ui, e),
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_capture_start(move || {
            let ui = weak.unwrap();
            match windows::cursor_position() {
                Ok((x,y)) => {
                    ui.set_x_text(x.to_string().into()); ui.set_y_text(y.to_string().into());
                    set_status(&ui, format!("Origem capturada: ({x}, {y}). Clique em Aplicar alterações."));
                }
                Err(e) => set_status(&ui, e),
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_capture_end(move || {
            let ui = weak.unwrap();
            match windows::cursor_position() {
                Ok((x,y)) => {
                    ui.set_x2_text(x.to_string().into()); ui.set_y2_text(y.to_string().into());
                    set_status(&ui, format!("Destino capturado: ({x}, {y}). Clique em Aplicar alterações."));
                }
                Err(e) => set_status(&ui, e),
            }
        });
    }
    {
        let weak = ui.as_weak(); let state = state.clone();
        ui.on_save_profile(move || {
            let ui = weak.unwrap();
            match cycles_from_ui(&ui) {
                Ok(cycles) => {
                    state.lock().unwrap().cycles = cycles;
                    autosave(&ui, &state, "Perfil salvo em %APPDATA%\\AutoClickPro\\profile.json");
                }
                Err(e) => set_status(&ui, e),
            }
        });
    }
    {
        let weak = ui.as_weak(); let state = state.clone(); let control = control.clone(); let overlays = overlays.clone();
        ui.on_load_profile(move || {
            let ui = weak.unwrap();
            if control.is_active() { set_status(&ui, "Pare a sequência antes de carregar."); return; }
            match profile::load() {
                Ok(p) => { load_into_ui(&ui, &state, &overlays, p); set_status(&ui, "Perfil carregado."); }
                Err(e) => set_status(&ui, e),
            }
        });
    }
    {
        let weak = ui.as_weak(); let state = state.clone(); let control = control.clone(); let overlays = overlays.clone();
        ui.on_play_toggle(move || {
            let ui = weak.unwrap();
            if control.is_active() { control.stop(); set_status(&ui, "Parando com segurança..."); return; }
            let cycles = match cycles_from_ui(&ui) {
                Ok(n) => n,
                Err(e) => { set_status(&ui, e); return; }
            };
            let snapshot = {
                let mut p = state.lock().unwrap();
                p.cycles = cycles;
                p.clone()
            };
            let _ = profile::save(&snapshot);
            // Importante: escondemos as miras ANTES de iniciar a thread.
            // Caso contrário o primeiro clique pode atingir a própria mira.
            let previously_visible = overlays.borrow().is_visible();
            overlays.borrow_mut().set_visible(false);
            let end_weak = weak.clone();
            let result = control.start(snapshot, move |outcome, interrupted| {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = end_weak.upgrade() {
                        ui.set_running(false);
                        match outcome {
                            Ok(_) if interrupted => set_status(&ui, "Sequência interrompida."),
                            Ok(_) => set_status(&ui, "Sequência concluída."),
                            Err(e) => set_status(&ui, format!("Erro no motor: {e}")),
                        }
                    }
                });
            });
            match result {
                Ok(()) => {
                    ui.set_running(true);
                    set_status(&ui, "Executando ações; miras ocultadas para não receber cliques.");
                }
                Err(e) => {
                    overlays.borrow_mut().set_visible(previously_visible);
                    set_status(&ui, e);
                }
            }
        });
    }

    {
        let weak = ui.as_weak(); let overlays = overlays.clone();
        ui.on_toggle_targets(move || {
            let ui = weak.unwrap();
            if ui.get_running() { return; }
            let visible = !ui.get_targets_visible();
            ui.set_targets_visible(visible);
            overlays.borrow_mut().set_visible(visible);
            set_status(&ui, if visible { "Miras visíveis: arraste para posicionar." } else { "Miras ocultas (F6 para mostrar)." });
        });
    }

    // Um único timer roda na thread de UI: sincroniza arrastos com a lista de
    // ações e salva após a mira ficar parada por alguns ciclos.
    let ticker = slint::Timer::default();
    {
        let weak = ui.as_weak(); let overlays = overlays.clone();
        let state = state.clone(); let control = control.clone();
        let last_change = Cell::new(None::<std::time::Instant>);
        ticker.start(slint::TimerMode::Repeated, Duration::from_millis(130), move || {
            let Some(ui) = weak.upgrade() else { return; };
            if control.is_active() { return; }
            // A conclusão pode atualizar `running` antes deste timer. A
            // visibilidade real das miras é restaurada independentemente disso.
            if overlays.borrow().is_visible() != ui.get_targets_visible() {
                overlays.borrow_mut().set_visible(ui.get_targets_visible());
            }
            if ui.get_running() { ui.set_running(false); }
            let moved = {
                let mut profile = state.lock().unwrap();
                overlays.borrow_mut().pull_positions(&mut profile)
            };
            if moved {
                last_change.set(Some(std::time::Instant::now()));
                let selected = ui.get_selected_index();
                refresh_list(&ui, &state);
                select_step(&ui, &state, selected);
            }
            if let Some(when) = last_change.get() {
                if when.elapsed() >= Duration::from_millis(450) {
                    autosave(&ui, &state, "Miras reposicionadas; coordenadas salvas.");
                    last_change.set(None);
                }
            }
        });
    }

    // Polling de borda: uma execução por pressionamento, nunca repete enquanto a tecla está segurada.
    // A thread nunca modifica Slint diretamente: apenas agenda callbacks na thread da interface.
    let keyboard_exit = exit.clone();
    let hotkey_weak = ui.as_weak();
    let keyboard_control = control.clone();
    let keyboard_thread = thread::spawn(move || {
        let vkeys = [0x74, 0x75, 0x77, 0x78, 0x1B]; // F5, F6, F8, F9, ESC
        let mut was_down = [false; 5];
        while !keyboard_exit.load(Ordering::SeqCst) {
            for (i, vk) in vkeys.iter().enumerate() {
                let down = windows::key_down(*vk);
                if down && !was_down[i] {
                    if i == 4 {
                        keyboard_control.stop();
                    } else {
                        let weak = hotkey_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = weak.upgrade() {
                                match i {
                                    0 => ui.invoke_play_toggle(),
                                    1 => ui.invoke_toggle_targets(),
                                    2 => ui.invoke_capture_start(),
                                    3 => ui.invoke_capture_end(),
                                    _ => (),
                                }
                            }
                        });
                    }
                }
                was_down[i] = down;
            }
            thread::sleep(Duration::from_millis(35));
        }
    });

    let run_result = ui.run();
    control.stop();
    exit.store(true, Ordering::SeqCst);
    let _ = keyboard_thread.join();
    // O usuário pode fechar a interface enquanto o mouse está pressionado num arrasto.
    // Aguardamos o worker encerrar e liberar o botão antes de terminar o processo.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while control.is_active() && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    run_result?;
    Ok(())
}
