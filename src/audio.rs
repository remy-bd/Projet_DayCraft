use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use std::f32::consts::TAU;

const RATE: u32 = 22_050;
const EFFECTS: [Effect; 14] = [
    Effect::Step,
    Effect::Break,
    Effect::Place,
    Effect::Hurt,
    Effect::Craft,
    Effect::Click,
    Effect::Gunshot,
    Effect::RocketLaunch,
    Effect::Explosion,
    Effect::MagazineOut,
    Effect::MagazineIn,
    Effect::Bolt,
    Effect::DryFire,
    Effect::C4Explosion,
];

#[derive(Clone, Copy)]
pub enum Effect {
    Step,
    Break,
    Place,
    Hurt,
    Craft,
    Click,
    Gunshot,
    RocketLaunch,
    Explosion,
    MagazineOut,
    MagazineIn,
    Bolt,
    DryFire,
    C4Explosion,
}

pub struct Audio {
    pub enabled: bool,
    sounds: [Option<Sound>; 14],
    weapons: Vec<[Option<Sound>; 4]>,
}

#[derive(Clone, Copy)]
pub enum WeaponSound {
    Shot,
    Open,
    Insert,
    Bolt,
}

impl Audio {
    pub async fn new() -> Self {
        let mut sounds = std::array::from_fn(|_| None);
        for effect in EFFECTS {
            sounds[effect as usize] = load_sound_from_bytes(&wav(effect)).await.ok();
        }
        let mut weapons = Vec::new();
        for assets in crate::weapon_audio::WEAPON_AUDIO {
            let mut bank = std::array::from_fn(|_| None);
            for (i, bytes) in assets.into_iter().enumerate() {
                bank[i] = load_sound_from_bytes(bytes).await.ok();
            }
            weapons.push(bank);
        }
        Self {
            enabled: true,
            sounds,
            weapons,
        }
    }

    pub fn weapon(&self, item: crate::game::Item, event: WeaponSound) {
        if !self.enabled {
            return;
        }
        let Some(i) = crate::weapons::GUNS.iter().position(|&g| g == item) else {
            return;
        };
        if let Some(sound) = &self.weapons[i][event as usize] {
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume: if matches!(event, WeaponSound::Shot) {
                        if matches!(
                            item,
                            crate::game::Item::M200
                                | crate::game::Item::Tundra
                                | crate::game::Item::Aw50
                        ) {
                            0.98
                        } else if item == crate::game::Item::Svd {
                            0.85
                        } else {
                            0.64
                        }
                    } else {
                        0.56
                    },
                },
            );
        }
    }

    pub fn assert_weapon_bank(&self) {
        assert!(
            self.sounds.iter().all(Option::is_some),
            "An effect sound failed to load"
        );
        assert_eq!(self.weapons.len(), 12);
        assert!(
            self.weapons.iter().flatten().all(Option::is_some),
            "A weapon sound failed to load"
        );
    }

    pub fn play(&self, effect: Effect) {
        if !self.enabled {
            return;
        }
        if let Some(sound) = &self.sounds[effect as usize] {
            let volume = match effect {
                Effect::Step => 0.28,
                Effect::Hurt => 0.45,
                Effect::Gunshot | Effect::RocketLaunch => 0.45,
                Effect::Explosion => 0.85,
                Effect::C4Explosion => 0.98,
                _ => 0.35,
            };
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume,
                },
            );
        }
    }
}

fn duration_ms(effect: Effect) -> u32 {
    match effect {
        Effect::Step => 70,
        Effect::Break => 150,
        Effect::Place => 95,
        Effect::Hurt => 180,
        Effect::Craft => 250,
        Effect::Click => 35,
        Effect::Gunshot => 110,
        Effect::RocketLaunch => 260,
        Effect::Explosion => 1500,
        Effect::C4Explosion => 2100,
        Effect::MagazineOut => 130,
        Effect::MagazineIn => 150,
        Effect::Bolt => 120,
        Effect::DryFire => 45,
    }
}

/// Small PCM16 WAVs need no decoder assets and work on native and web audio backends.
fn wav(effect: Effect) -> Vec<u8> {
    let count = RATE * duration_ms(effect) / 1_000;
    let size = count * 2;
    let mut bytes = Vec::with_capacity((44 + size) as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(size + 36).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // Mono
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes()); // Block alignment
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&size.to_le_bytes());

    let mut random = 0x5eeda11u32.wrapping_add(effect as u32 * 7_919);
    let mut gravel = 0.0;
    for i in 0..count {
        let t = i as f32 / RATE as f32;
        random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (random >> 8) as f32 / 16_777_215.0 * 2.0 - 1.0;
        gravel += (noise - gravel) * 0.34;
        let tone = |hz: f32| (TAU * hz * t).sin();
        let signal = match effect {
            Effect::Step => (0.65 * gravel + 0.3 * tone(92.0)) * (-t * 48.0).exp(),
            Effect::Break => {
                let fragments = 0.65 + 0.35 * (TAU * 31.0 * t).sin().abs();
                (0.8 * gravel + 0.15 * tone(66.0)) * fragments * (-t * 19.0).exp()
            }
            Effect::Place => {
                (0.45 * tone(155.0) + 0.15 * tone(310.0) + 0.18 * gravel) * (-t * 38.0).exp()
            }
            Effect::Hurt => {
                let falling = (TAU * (220.0 * t - 310.0 * t * t)).sin();
                (0.45 * falling + 0.24 * gravel) * (-t * 18.0).exp()
            }
            Effect::Craft => {
                let first = tone(523.25) * (-t * 17.0).exp();
                let second_t = (t - 0.09).max(0.0);
                let second = (TAU * 783.99 * second_t).sin() * (-second_t * 20.0).exp();
                0.36 * first + 0.32 * second
            }
            Effect::Click => (0.36 * tone(1_250.0) + 0.12 * gravel) * (-t * 105.0).exp(),
            Effect::MagazineOut => (0.35 * gravel + 0.25 * tone(310.)) * (-t * 32.).exp(),
            Effect::MagazineIn => (0.55 * gravel + 0.25 * tone(145.)) * (-t * 38.).exp(),
            Effect::Bolt => (0.60 * noise + 0.20 * tone(680.)) * (-t * 40.).exp(),
            Effect::DryFire => (0.38 * tone(890.) + 0.22 * noise) * (-t * 95.).exp(),
            Effect::Gunshot => (0.75 * noise + 0.3 * tone(105.0)) * (-t * 45.0).exp(),
            Effect::RocketLaunch => {
                let rising = (TAU * (130.0 * t + 420.0 * t * t)).sin();
                (0.55 * gravel + 0.28 * rising) * (-t * 9.0).exp()
            }
            Effect::Explosion | Effect::C4Explosion => {
                let heavy = matches!(effect, Effect::C4Explosion);
                let bass = (TAU * ((if heavy { 43. } else { 65. }) * t - 8. * t * t)).sin()
                    * (-t * 4.).exp();
                let attack = noise * (-t * 85.).exp() * 0.55;
                let rubble = gravel * (-t * 3.2).exp() * 0.38;
                let echo = if t > 0.14 {
                    gravel * (-(t - 0.14) * 8.).exp() * 0.22
                } else {
                    0.
                };
                attack + bass * 0.52 + rubble + echo
            }
        };
        let attack = (t / 0.002).min(1.0);
        let release = ((count - 1 - i) as f32 / (RATE as f32 * 0.005)).min(1.0);
        let sample = (signal * attack * release).clamp(-0.95, 0.95);
        bytes.extend_from_slice(&((sample * i16::MAX as f32) as i16).to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stereo_weapon_assets_are_distinct_have_headroom_and_clean_edges() {
        let mut signatures = std::collections::HashSet::new();
        for bank in crate::weapon_audio::WEAPON_AUDIO {
            for bytes in bank {
                assert_eq!(&bytes[..4], b"RIFF");
                assert_eq!(&bytes[8..12], b"WAVE");
                assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 2);
                assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 44100);
                assert_eq!(u16::from_le_bytes(bytes[34..36].try_into().unwrap()), 16);
                let samples: Vec<_> = bytes[44..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| i16::from_le_bytes(*b))
                    .collect();
                assert!(samples.len() > 20000);
                assert_eq!(&samples[..2], &[0, 0]);
                assert_eq!(&samples[samples.len() - 2..], &[0, 0]);
                assert!(samples.iter().all(|s| s.unsigned_abs() < 30000));
                assert!(
                    signatures.insert(bytes[44..4044].to_vec()),
                    "Two weapon events share the same audio"
                );
            }
        }
    }

    #[test]
    fn generated_wavs_are_valid_deterministic_pcm_with_smooth_edges() {
        for effect in EFFECTS {
            let bytes = wav(effect);
            let u16_at = |at| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
            let u32_at = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            assert_eq!(&bytes[..4], b"RIFF");
            assert_eq!(&bytes[8..16], b"WAVEfmt ");
            assert_eq!(&bytes[36..40], b"data");
            assert_eq!(u32_at(4) as usize + 8, bytes.len());
            assert_eq!(u32_at(16), 16);
            assert_eq!(u16_at(20), 1);
            assert_eq!(u16_at(22), 1);
            assert_eq!(u32_at(24), RATE);
            assert_eq!(u32_at(28), RATE * u16_at(32) as u32);
            assert_eq!(u16_at(32), 2);
            assert_eq!(u16_at(34), 16);
            assert_eq!(u32_at(40) as usize, bytes.len() - 44);
            let samples: Vec<_> = bytes[44..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| i16::from_le_bytes(*pair))
                .collect();
            assert_eq!(samples.len(), (RATE * duration_ms(effect) / 1_000) as usize);
            assert_eq!(samples.first(), Some(&0));
            assert_eq!(samples.last(), Some(&0));
            assert!(samples.iter().any(|sample| sample.abs() > 1_000));
            assert!(samples.iter().all(|sample| sample.abs() < i16::MAX));
            assert_eq!(bytes, wav(effect));
        }
    }
}

#[cfg(test)]
mod sniper_sound_tests {
    #[test]
    fn sniper_attacks_are_louder_and_have_a_long_nonempty_echo_tail() {
        let rms = |bytes: &[u8], start: f32, end: f32| {
            let (start, end) = (
                (start * 44100. * 4.) as usize + 44,
                (end * 44100. * 4.) as usize + 44,
            );
            let samples = &bytes[start..end.min(bytes.len())];
            let sum = samples
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| {
                    let s = i16::from_le_bytes(*b) as f64;
                    s * s
                })
                .sum::<f64>();
            (sum / (samples.len() / 2) as f64).sqrt()
        };
        let banks = crate::weapon_audio::WEAPON_AUDIO;
        let rifle = (0..6)
            .chain(std::iter::once(11))
            .map(|i| rms(banks[i][0], 0., 0.2))
            .fold(0., f64::max);
        for i in [7, 8, 9, 10] {
            let seconds = (banks[i][0].len() - 44) as f32 / (44100. * 4.);
            assert!(seconds >= if i == 10 { 3.5 } else { 5.5 });
            assert!(rms(banks[i][0], 0., 0.2) > rifle * 1.3);
            assert!(rms(banks[i][0], 2., 3.) > 60., "No audible echo tail");
        }
    }
}
