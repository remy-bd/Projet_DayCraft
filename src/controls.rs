//! Keyboard bindings and local preferences, independent of the world save.
use macroquad::prelude::KeyCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io, path::Path};

#[derive(Default)]
pub struct FocusKeys {
    blocked: HashSet<KeyCode>,
}

impl FocusKeys {
    pub fn lost_focus(&mut self, held: impl IntoIterator<Item = KeyCode>) {
        self.blocked.extend(held);
    }

    pub fn fresh_events(&mut self, events: impl IntoIterator<Item = KeyCode>) {
        for key in events {
            self.blocked.remove(&key);
        }
    }

    pub fn allows(&self, key: KeyCode) -> bool {
        !self.blocked.contains(&key)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Action {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Sprint,
    Sneak,
    Inventory,
    Fly,
    RemoveFluid,
    PickBlock,
    Reload,
    FireMode,
    Inspect,
    Drop,
    Optic,
    Creative,
}

pub const ACTIONS: [Action; 17] = [
    Action::Forward,
    Action::Backward,
    Action::Left,
    Action::Right,
    Action::Jump,
    Action::Sprint,
    Action::Sneak,
    Action::Inventory,
    Action::Fly,
    Action::RemoveFluid,
    Action::PickBlock,
    Action::Reload,
    Action::FireMode,
    Action::Inspect,
    Action::Drop,
    Action::Optic,
    Action::Creative,
];

impl Action {
    pub fn name(self) -> &'static str {
        match self {
            Self::Forward => "Avancer",
            Self::Backward => "Reculer",
            Self::Left => "Aller à gauche",
            Self::Right => "Aller à droite",
            Self::Jump => "Sauter / monter / nager",
            Self::Sprint => "Courir / vol rapide",
            Self::Sneak => "Prudence / descendre en vol",
            Self::Inventory => "Inventaire",
            Self::Fly => "Activer le vol (créatif)",
            Self::RemoveFluid => "Retirer un liquide (créatif)",
            Self::PickBlock => "Copier le bloc (créatif)",
            Self::Reload => "Recharger l'arme",
            Self::FireMode => "Mode auto / semi / rafale",
            Self::Inspect => "Inspecter l'arme",
            Self::Drop => "Jeter (Ctrl : pile entière)",
            Self::Optic => "Changer d'optique",
            Self::Creative => "Menu créatif",
        }
    }
}

pub const BINDABLE_KEYS: &[KeyCode] = &[
    KeyCode::A,
    KeyCode::B,
    KeyCode::C,
    KeyCode::D,
    KeyCode::E,
    KeyCode::F,
    KeyCode::G,
    KeyCode::H,
    KeyCode::I,
    KeyCode::J,
    KeyCode::K,
    KeyCode::L,
    KeyCode::M,
    KeyCode::N,
    KeyCode::O,
    KeyCode::P,
    KeyCode::Q,
    KeyCode::R,
    KeyCode::S,
    KeyCode::T,
    KeyCode::U,
    KeyCode::V,
    KeyCode::W,
    KeyCode::X,
    KeyCode::Y,
    KeyCode::Z,
    KeyCode::Space,
    KeyCode::Tab,
    KeyCode::Enter,
    KeyCode::Backspace,
    KeyCode::LeftShift,
    KeyCode::LeftControl,
    KeyCode::LeftAlt,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Insert,
    KeyCode::Delete,
    KeyCode::Comma,
    KeyCode::Period,
    KeyCode::Slash,
    KeyCode::Semicolon,
    KeyCode::Apostrophe,
    KeyCode::LeftBracket,
    KeyCode::RightBracket,
    KeyCode::Backslash,
    KeyCode::Minus,
    KeyCode::Equal,
    KeyCode::GraveAccent,
    KeyCode::F2,
    KeyCode::F4,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F12,
];

pub fn canonical_key(key: KeyCode) -> KeyCode {
    match key {
        KeyCode::RightShift => KeyCode::LeftShift,
        KeyCode::RightControl => KeyCode::LeftControl,
        KeyCode::RightAlt => KeyCode::LeftAlt,
        _ => key,
    }
}

pub fn key_name(key: KeyCode, azerty: bool) -> String {
    let key = canonical_key(key);
    match key {
        KeyCode::Space => "Espace".into(),
        KeyCode::LeftShift => "Maj".into(),
        KeyCode::LeftControl => "Ctrl".into(),
        KeyCode::LeftAlt => "Alt".into(),
        KeyCode::Enter => "Entrée".into(),
        KeyCode::Backspace => "Retour arrière".into(),
        KeyCode::Up => "↑".into(),
        KeyCode::Down => "↓".into(),
        KeyCode::Left => "←".into(),
        KeyCode::Right => "→".into(),
        KeyCode::W if cfg!(windows) && azerty => "Z".into(),
        KeyCode::A if cfg!(windows) && azerty => "Q".into(),
        KeyCode::Q if cfg!(windows) && azerty => "A".into(),
        KeyCode::Z if cfg!(windows) && azerty => "W".into(),
        KeyCode::Semicolon if cfg!(windows) && azerty => "M".into(),
        KeyCode::M if cfg!(windows) && azerty => ",".into(),
        KeyCode::Comma if cfg!(windows) && azerty => ";".into(),
        _ => format!("{key:?}"),
    }
}

pub fn default_bindings(azerty: bool) -> [KeyCode; 17] {
    let (forward, left) = if cfg!(windows) || !azerty {
        (KeyCode::W, KeyCode::A)
    } else {
        (KeyCode::Z, KeyCode::Q)
    };
    [
        forward,
        KeyCode::S,
        left,
        KeyCode::D,
        KeyCode::Space,
        KeyCode::LeftShift,
        KeyCode::LeftControl,
        KeyCode::E,
        KeyCode::F,
        KeyCode::X,
        KeyCode::V,
        KeyCode::R,
        KeyCode::B,
        KeyCode::I,
        KeyCode::G,
        KeyCode::O,
        KeyCode::C,
    ]
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub bindings: Vec<String>,
    pub azerty: bool,
    pub sensitivity: f32,
    pub invert_y: bool,
    pub toggle_sprint: bool,
    pub radius: i32,
    pub sound: bool,
    pub hints: bool,
    pub shaders: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            bindings: default_bindings(true).map(|k| format!("{k:?}")).to_vec(),
            azerty: true,
            sensitivity: 0.0025,
            invert_y: false,
            toggle_sprint: false,
            radius: 4,
            sound: true,
            hints: true,
            shaders: true,
        }
    }
}

impl Preferences {
    pub fn decode_bindings(&self) -> Option<[KeyCode; 17]> {
        let mut keys = default_bindings(self.azerty);
        if self.bindings.len() != keys.len()
            && self.bindings.len() != 11
            && self.bindings.len() != 14
            && self.bindings.len() != 16
        {
            return None;
        }
        for (index, name) in self.bindings.iter().enumerate() {
            let key = BINDABLE_KEYS
                .iter()
                .find(|key| format!("{key:?}") == *name)?;
            if keys[..index].contains(key) {
                return None;
            }
            keys[index] = *key;
        }
        // Older preferences keep every existing binding, even if R/B/I were used.
        for index in self.bindings.len()..keys.len() {
            if keys[..index].contains(&keys[index]) {
                keys[index] = *BINDABLE_KEYS.iter().find(|k| !keys[..index].contains(k))?;
            }
        }
        Some(keys)
    }

    pub fn load(path: &Path) -> Option<Self> {
        let preferences: Self = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        preferences.decode_bindings()?;
        if !preferences.sensitivity.is_finite()
            || !(0.0005..=0.006).contains(&preferences.sensitivity)
            || !(2..=6).contains(&preferences.radius)
        {
            return None;
        }
        Some(preferences)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temporary, path)
    }
}

pub fn rebind(keys: &mut [KeyCode; 17], action: Action, key: KeyCode) -> bool {
    let key = canonical_key(key);
    if !BINDABLE_KEYS.contains(&key) {
        return false;
    }
    let index = action as usize;
    if let Some(other) = keys.iter().position(|binding| *binding == key) {
        keys.swap(index, other);
    } else {
        keys[index] = key;
    }
    true
}

pub fn slot_key(key: KeyCode) -> Option<usize> {
    let keys = [
        KeyCode::Key1,
        KeyCode::Key2,
        KeyCode::Key3,
        KeyCode::Key4,
        KeyCode::Key5,
        KeyCode::Key6,
        KeyCode::Key7,
        KeyCode::Key8,
        KeyCode::Key9,
    ];
    let pad = [
        KeyCode::Kp1,
        KeyCode::Kp2,
        KeyCode::Kp3,
        KeyCode::Kp4,
        KeyCode::Kp5,
        KeyCode::Kp6,
        KeyCode::Kp7,
        KeyCode::Kp8,
        KeyCode::Kp9,
    ];
    keys.iter()
        .position(|k| *k == key)
        .or_else(|| pad.iter().position(|k| *k == key))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_keys_after_focus_loss_require_fresh_events() {
        let mut keys = FocusKeys::default();
        keys.lost_focus([KeyCode::W, KeyCode::LeftShift]);
        keys.fresh_events([]);
        assert!(!keys.allows(KeyCode::W));
        assert!(!keys.allows(KeyCode::LeftShift));
        keys.fresh_events([KeyCode::RightShift]);
        assert!(keys.allows(KeyCode::RightShift));
        assert!(!keys.allows(KeyCode::LeftShift));
        keys.fresh_events([KeyCode::W]);
        assert!(keys.allows(KeyCode::W));
    }
    #[test]
    fn bindings_roundtrip_swap_conflicts_and_protect_shortcuts() {
        let mut keys = default_bindings(true);
        assert!(rebind(&mut keys, Action::Inventory, KeyCode::W));
        assert_eq!(keys[Action::Forward as usize], KeyCode::E);
        assert_eq!(keys[Action::Inventory as usize], KeyCode::W);
        assert!(!rebind(&mut keys, Action::Jump, KeyCode::Escape));
        assert!(!rebind(&mut keys, Action::Jump, KeyCode::Key1));
        assert!(!rebind(&mut keys, Action::Jump, KeyCode::F5));
        let mut saved = Preferences {
            bindings: keys.map(|k| format!("{k:?}")).to_vec(),
            ..Default::default()
        };
        let decoded: Preferences =
            serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(decoded.decode_bindings(), Some(keys));
        let mut legacy = serde_json::to_value(&saved).unwrap();
        legacy.as_object_mut().unwrap().remove("shaders");
        assert!(
            serde_json::from_value::<Preferences>(legacy)
                .unwrap()
                .shaders
        );
        saved.bindings[0] = saved.bindings[1].clone();
        assert!(saved.decode_bindings().is_none());
        saved.bindings[0] = "Escape".into();
        assert!(saved.decode_bindings().is_none());
        assert_eq!(slot_key(KeyCode::Kp9), Some(8));
        assert_eq!(slot_key(KeyCode::Key1), Some(0));
        assert_eq!(slot_key(KeyCode::Key0), None);
        assert_eq!(canonical_key(KeyCode::RightShift), KeyCode::LeftShift);
    }

    #[test]
    fn old_eleven_key_settings_preserve_custom_keys_and_add_unique_weapon_controls() {
        let mut old = Preferences::default();
        old.bindings.truncate(11);
        old.bindings[4] = "R".into();
        old.bindings[5] = "B".into();
        old.bindings[6] = "I".into();
        let keys = old.decode_bindings().unwrap();
        assert_eq!(&keys[4..7], &[KeyCode::R, KeyCode::B, KeyCode::I]);
        assert_eq!(keys.into_iter().collect::<HashSet<_>>().len(), 17);
    }
    #[test]
    fn old_sixteen_key_settings_preserve_bindings_and_add_a_distinct_creative_key() {
        for occupied in [false, true] {
            let mut old = Preferences::default();
            old.bindings.truncate(16);
            old.bindings
                .swap(Action::Forward as usize, Action::Inventory as usize);
            if occupied {
                old.bindings[Action::Fly as usize] = "C".into();
            }
            let keys = old.decode_bindings().unwrap();
            for (index, saved) in old.bindings.iter().enumerate() {
                assert_eq!(&format!("{:?}", keys[index]), saved);
            }
            let creative = keys[Action::Creative as usize];
            assert!(!keys[..16].contains(&creative));
            assert_eq!(creative == KeyCode::C, !occupied);
            assert_eq!(keys.into_iter().collect::<HashSet<_>>().len(), 17);
            old.bindings = keys.map(|k| format!("{k:?}")).to_vec();
            assert_eq!(old.decode_bindings(), Some(keys));
        }
    }
    #[test]
    #[cfg(windows)]
    fn azerty_labels_match_physical_windows_keys() {
        assert_eq!(default_bindings(true)[0], KeyCode::W);
        assert_eq!(key_name(KeyCode::W, true), "Z");
        assert_eq!(key_name(KeyCode::A, true), "Q");
        assert_eq!(key_name(KeyCode::W, false), "W");
    }
}
