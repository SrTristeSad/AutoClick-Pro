use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Click,
    DoubleClick,
    Drag,
}

impl StepKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Click => "Clique esquerdo",
            Self::DoubleClick => "Clique duplo",
            Self::Drag => "Arrastar mouse",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    pub x: i32,
    pub y: i32,
    pub x2: i32,
    pub y2: i32,
    pub delay_ms: u32,
    pub duration_ms: u32,
}

impl Step {
    pub fn new(kind: StepKind, x: i32, y: i32) -> Self {
        Self { kind, x, y, x2: x.saturating_add(120), y2: y, delay_ms: 200, duration_ms: 500 }
    }

    fn validate(&self) -> Result<(), String> {
        if !(20..=60_000).contains(&self.delay_ms) {
            return Err("Espera deve ficar entre 20 e 60000 ms.".into());
        }
        if !(10..=30_000).contains(&self.duration_ms) {
            return Err("Arrasto deve durar de 10 a 30000 ms.".into());
        }
        for n in [self.x, self.y, self.x2, self.y2] {
            if !(-100_000..=100_000).contains(&n) {
                return Err("Coordenada fora do intervalo permitido.".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub version: u32,
    pub cycles: u32,
    pub steps: Vec<Step>,
}

impl Default for Profile {
    fn default() -> Self {
        Self { version: 1, cycles: 0, steps: Vec::new() }
    }
}

impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 { return Err("Versão de perfil não suportada.".into()); }
        if self.steps.len() > 200 { return Err("Limite de 200 ações por perfil.".into()); }
        if self.cycles > 100_000 { return Err("Número máximo de ciclos: 100000.".into()); }
        for step in &self.steps { step.validate()?; }
        Ok(())
    }
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("AutoClickPro").join("profile.json")
}

pub fn load() -> Result<Profile, String> {
    let location = path();
    if !location.exists() { return Ok(Profile::default()); }
    load_file(&location)
}

pub fn load_file(location: &Path) -> Result<Profile, String> {
    let metadata = fs::metadata(location).map_err(|err| format!("Não foi possível consultar perfil: {err}"))?;
    if metadata.len() > 2_000_000 { return Err("Perfil excede o limite de 2 MB.".into()); }
    let bytes = fs::read(location).map_err(|err| format!("Não foi possível ler perfil: {err}"))?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|err| format!("JSON inválido: {err}"))?;
    let profile = if value.get("acoes").is_some() {
        import_legacy(&value)?
    } else {
        serde_json::from_value(value).map_err(|err| format!("Perfil inválido: {err}"))?
    };
    profile.validate()?;
    Ok(profile)
}

pub fn save(profile: &Profile) -> Result<(), String> {
    save_file(profile, &path())
}

pub fn save_file(profile: &Profile, location: &Path) -> Result<(), String> {
    profile.validate()?;
    if let Some(parent) = location.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("Falha ao criar pasta: {err}"))?;
    }
    let data = serde_json::to_vec_pretty(profile).map_err(|err| err.to_string())?;
    // Arquivo temporário na mesma pasta: reduz o risco de um JSON truncado
    // caso o processo seja interrompido durante a gravação.
    let temp = location.with_extension("json.tmp");
    fs::write(&temp, data).map_err(|err| format!("Falha ao gravar temporário: {err}"))?;
    fs::rename(&temp, location).map_err(|err| {
        let _ = fs::remove_file(&temp);
        format!("Falha ao finalizar salvamento: {err}")
    })
}

// Compatibilidade com os arquivos JSON exportados pela versão Python.
fn import_legacy(value: &Value) -> Result<Profile, String> {
    let cycles = value.get("ciclos")
        .and_then(|v| v.as_str().and_then(|s| s.parse::<u32>().ok()).or_else(|| v.as_u64().and_then(|n| n.try_into().ok())))
        .unwrap_or(0);
    let actions = value.get("acoes").and_then(Value::as_array)
        .ok_or_else(|| "JSON Python sem lista de ações.".to_string())?;
    let mut profile = Profile { cycles, ..Profile::default() };
    for item in actions {
        let kind = match item.get("tipo").and_then(Value::as_str) {
            Some("clique") => {
                if item.pointer("/config/acao").and_then(Value::as_str) == Some("Clique Duplo") {
                    StepKind::DoubleClick
                } else { StepKind::Click }
            }
            Some("swipe") => StepKind::Drag,
            _ => return Err("Tipo de ação do JSON antigo não reconhecido.".into()),
        };
        let num = |key: &str| item.get(key).and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok()).unwrap_or(0);
        let (x, y, x2, y2) = if kind == StepKind::Drag {
            (num("x1").saturating_add(22), num("y1").saturating_add(22),
             num("x2").saturating_add(22), num("y2").saturating_add(22))
        } else { (num("x").saturating_add(22), num("y").saturating_add(22), 0, 0) };
        let seconds_to_ms = |key: &str, default: u32| -> u32 {
            let seconds = item.pointer(&format!("/config/{key}"))
                .and_then(Value::as_f64).unwrap_or(default as f64 / 1000.0);
            if !seconds.is_finite() || seconds < 0.0 { default } else { (seconds * 1000.0).round().clamp(20.0, 60_000.0) as u32 }
        };
        profile.steps.push(Step {
            kind, x, y, x2, y2,
            delay_ms: seconds_to_ms("espera", 200),
            duration_ms: seconds_to_ms("duracao", 500).clamp(10, 30_000),
        });
    }
    profile.validate()?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_round_trip() {
        let profile = Profile { version: 1, cycles: 2, steps: vec![Step::new(StepKind::Click, 300, 400)] };
        let json = serde_json::to_string(&profile).unwrap();
        let decoded: Profile = serde_json::from_str(&json).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded.steps[0].x, 300);
    }

    #[test]
    fn saves_and_loads_profile_file() {
        let filename = format!("autoclick-pro-test-{}-{}.json", std::process::id(), 301);
        let location = std::env::temp_dir().join(filename);
        let mut original = Profile::default();
        original.cycles = 5;
        original.steps.push(Step::new(StepKind::Drag, 140, 240));
        save_file(&original, &location).unwrap();
        let recovered = load_file(&location).unwrap();
        assert_eq!(recovered.cycles, 5);
        assert_eq!(recovered.steps[0].kind, StepKind::Drag);
        assert_eq!(recovered.steps[0].x2, 260);
        // Segunda gravação no mesmo caminho exercita a substituição do arquivo.
        save_file(&recovered, &location).unwrap();
        let _ = std::fs::remove_file(location);
    }

    #[test]
    fn rejects_out_of_range_steps() {
        let mut p = Profile::default();
        let mut step = Step::new(StepKind::Click, 1, 1);
        step.delay_ms = 0;
        p.steps.push(step);
        assert!(p.validate().is_err());
    }

    #[test]
    fn imports_legacy_drag_both_endpoints() {
        let value = serde_json::json!({"ciclos":"1","acoes":[
            {"tipo":"swipe","x1":10,"y1":20,"x2":110,"y2":120,
             "config":{"duracao":0.5,"espera":0.2}}
        ]});
        let p = import_legacy(&value).unwrap();
        assert_eq!((p.steps[0].x, p.steps[0].y), (32, 42));
        assert_eq!((p.steps[0].x2, p.steps[0].y2), (132, 142));
        assert_eq!(p.steps[0].duration_ms, 500);
    }

    #[test]
    fn imports_python_center_coordinates() {
        let value = serde_json::json!({"ciclos": "2", "acoes": [
            {"tipo":"clique", "x":100, "y":120, "config":{"acao":"Clique Duplo", "espera":0.2}}
        ]});
        let profile = import_legacy(&value).unwrap();
        assert_eq!(profile.steps[0].kind, StepKind::DoubleClick);
        assert_eq!(profile.steps[0].x, 122);
        assert_eq!(profile.steps[0].delay_ms, 200);
    }
}
