use crate::profile::{Profile, Step, StepKind};
use crate::windows;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct Controller {
    active: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl Controller {
    pub fn is_active(&self) -> bool { self.active.load(Ordering::SeqCst) }
    pub fn stop(&self) { self.stop.store(true, Ordering::SeqCst); }

    pub fn start(&self, profile: Profile, callback: impl FnOnce(Result<(), String>, bool) + Send + 'static) -> Result<(), String> {
        profile.validate()?;
        if profile.steps.is_empty() { return Err("Adicione pelo menos uma ação antes de iniciar.".into()); }
        if self.active.swap(true, Ordering::SeqCst) { return Err("Já existe uma sequência em execução.".into()); }
        self.stop.store(false, Ordering::SeqCst);
        let active = self.active.clone();
        let stop = self.stop.clone();
        match thread::Builder::new().name("autoclick-worker".into()).spawn(move || {
            let outcome = run_sequence(&profile, &stop);
            let interrupted = stop.load(Ordering::SeqCst);
            active.store(false, Ordering::SeqCst);
            callback(outcome, interrupted);
        }) {
            Ok(_) => Ok(()),
            Err(error) => {
                // Não deixe o aplicativo preso em "executando" caso o SO
                // recuse a criação da thread de automação.
                self.active.store(false, Ordering::SeqCst);
                self.stop.store(true, Ordering::SeqCst);
                Err(format!("Não foi possível iniciar a automação: {error}"))
            }
        }
    }
}

fn wait_interruptible(duration: Duration, stop: &AtomicBool) -> bool {
    let until = Instant::now() + duration;
    while Instant::now() < until {
        if stop.load(Ordering::SeqCst) { return false; }
        thread::sleep(until.saturating_duration_since(Instant::now()).min(Duration::from_millis(10)));
    }
    !stop.load(Ordering::SeqCst)
}

fn run_sequence(profile: &Profile, stop: &AtomicBool) -> Result<(), String> {
    let mut count = 0_u32;
    while !stop.load(Ordering::SeqCst) && (profile.cycles == 0 || count < profile.cycles) {
        for step in &profile.steps {
            if stop.load(Ordering::SeqCst) { break; }
            perform(step, stop)?;
            if !wait_interruptible(Duration::from_millis(step.delay_ms as u64), stop) { break; }
        }
        count = count.saturating_add(1);
    }
    Ok(())
}

fn perform(step: &Step, stop: &AtomicBool) -> Result<(), String> {
    match step.kind {
        StepKind::Click => windows::click(step.x, step.y),
        StepKind::DoubleClick => {
            windows::click(step.x, step.y)?;
            if wait_interruptible(Duration::from_millis(75), stop) {
                windows::click(step.x, step.y)?;
            }
            Ok(())
        }
        StepKind::Drag => {
            windows::move_cursor(step.x, step.y)?;
            windows::down()?;
            let points = (step.duration_ms / 10).clamp(1, 250);
            let mut outcome = Ok(());
            for n in 1..=points {
                if stop.load(Ordering::SeqCst) { break; }
                let t = n as f64 / points as f64;
                let x = step.x as f64 + (step.x2 - step.x) as f64 * t;
                let y = step.y as f64 + (step.y2 - step.y) as f64 * t;
                if let Err(err) = windows::move_cursor(x.round() as i32, y.round() as i32) {
                    outcome = Err(err);
                    break;
                }
                if !wait_interruptible(Duration::from_millis(step.duration_ms as u64 / points as u64), stop) { break; }
            }
            // Solta o botão mesmo em caso de erro ou parada de emergência.
            let released = windows::up();
            outcome.and(released)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interruptible_wait_stops_immediately() {
        let stop = AtomicBool::new(true);
        assert!(!wait_interruptible(Duration::from_secs(30), &stop));
    }

    #[test]
    fn empty_profile_cannot_activate_worker() {
        let controller = Controller::default();
        let result = controller.start(Profile::default(), |_, _| {});
        assert!(result.is_err());
        assert!(!controller.is_active());
    }
}
