//! Erzeugt die Platzhalter-Geräusche des Spiels als WAV-Dateien unter `assets/sounds/`.
//! Alles wird aus Tönen, Rauschen und Hüllkurven synthetisiert, ganz ohne Aufnahmen.
//! Jede Datei lässt sich später durch eine bessere mit demselben Namen ersetzen.
//!
//! Aufruf aus dem Projektordner: `cargo run -p sound_gen`

use std::f32::consts::TAU;
use std::fs;
use std::path::Path;

const RATE: u32 = 44_100;

/// Ein einfacher, fester Zufallsgenerator, damit jeder Lauf dieselben Dateien ergibt.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
    }
}

fn samples(seconds: f32) -> usize {
    (seconds * RATE as f32) as usize
}

fn time(i: usize) -> f32 {
    i as f32 / RATE as f32
}

/// Kurzer Anstieg, dann exponentielles Abklingen.
fn envelope(t: f32, attack: f32, decay: f32) -> f32 {
    if t < attack { t / attack } else { (-(t - attack) / decay).exp() }
}

/// Mischt `other` ab Sekunde `at` hinein.
fn mix_into(target: &mut Vec<f32>, other: &[f32], at: f32) {
    let start = samples(at);
    if target.len() < start + other.len() {
        target.resize(start + other.len(), 0.0);
    }
    for (i, s) in other.iter().enumerate() {
        target[start + i] += s;
    }
}

/// Ein dumpfer Schlag auf die Erde: tiefer Ton, der nach unten rutscht, und etwas Rauschen.
fn thump(volume: f32, seed: u64) -> Vec<f32> {
    let mut noise = Noise(seed);
    let mut phase = 0.0;
    let mut low = 0.0;
    (0..samples(0.3))
        .map(|i| {
            let t = time(i);
            let freq = 55.0 + 90.0 * (-t * 25.0).exp();
            phase += freq / RATE as f32;
            low += (noise.next() - low) * 0.08;
            let tone = (phase * TAU).sin() * envelope(t, 0.004, 0.09);
            let dust = low * envelope(t, 0.002, 0.03) * 1.5;
            (tone + dust) * volume
        })
        .collect()
}

/// Kratzen in der Erde: kurze Stöße gefilterten Rauschens.
fn scratch(seed: u64) -> Vec<f32> {
    let mut noise = Noise(seed);
    let mut out = Vec::new();
    for (k, at) in [0.0, 0.11, 0.22].into_iter().enumerate() {
        let mut band = 0.0;
        let mut prev = 0.0;
        let burst: Vec<f32> = (0..samples(0.09))
            .map(|i| {
                let t = time(i);
                let n = noise.next();
                // Grob bandbegrenzt: tiefes Rumpeln weg, ganz Hohes weg.
                band += (n - band) * 0.35;
                let high = band - prev;
                prev = band;
                high * envelope(t, 0.01, 0.03) * (0.35 - k as f32 * 0.05)
            })
            .collect();
        mix_into(&mut out, &burst, at);
    }
    out
}

/// Ein weicher Glockenton mit Obertönen.
fn bell(freq: f32, seconds: f32, volume: f32) -> Vec<f32> {
    (0..samples(seconds))
        .map(|i| {
            let t = time(i);
            let tone = (t * freq * TAU).sin() + 0.35 * (t * freq * 2.0 * TAU).sin() + 0.12 * (t * freq * 3.01 * TAU).sin();
            tone * envelope(t, 0.005, seconds * 0.35) * volume
        })
        .collect()
}

/// Mehrere Glockentöne nacheinander.
fn melody(notes: &[(f32, f32)], step: f32, volume: f32) -> Vec<f32> {
    let mut out = Vec::new();
    for (k, (freq, length)) in notes.iter().enumerate() {
        mix_into(&mut out, &bell(*freq, *length, volume), k as f32 * step);
    }
    out
}

/// Der Ruf des Bussards: ein langgezogenes, abfallendes „Hiiää“ mit leichtem Zittern.
fn buzzard_cry() -> Vec<f32> {
    let mut phase = 0.0;
    let length = 0.9;
    (0..samples(length))
        .map(|i| {
            let t = time(i);
            let glide = 2300.0 - 900.0 * (t / length).powf(0.7);
            let freq = glide * (1.0 + 0.02 * (t * 38.0 * TAU).sin());
            phase += freq / RATE as f32;
            let tone = (phase * TAU).sin() + 0.4 * (phase * 2.0 * TAU).sin() + 0.2 * (phase * 3.0 * TAU).sin();
            let shape = (t / 0.06).min(1.0) * ((length - t) / 0.35).clamp(0.0, 1.0);
            tone * shape * 0.22
        })
        .collect()
}

/// Rascheln im Gebüsch: Rauschen, das unregelmäßig an- und abschwillt.
fn rustle() -> Vec<f32> {
    let mut noise = Noise(7);
    let mut flutter = Noise(99);
    let mut band = 0.0;
    let mut prev = 0.0;
    let mut gate = 0.0;
    let length = 0.9;
    (0..samples(length))
        .map(|i| {
            let t = time(i);
            if i % 1200 == 0 {
                gate = 0.4 + 0.6 * flutter.next().abs();
            }
            let n = noise.next();
            band += (n - band) * 0.5;
            let high = band - prev;
            prev = band;
            let shape = (t / 0.1).min(1.0) * ((length - t) / 0.3).clamp(0.0, 1.0);
            high * gate * shape * 0.5
        })
        .collect()
}

/// Ein kurzes „Plopp“, wenn etwas gebaut oder geplant wird.
fn plop() -> Vec<f32> {
    let mut phase = 0.0;
    (0..samples(0.14))
        .map(|i| {
            let t = time(i);
            let freq = 300.0 + 500.0 * (-t * 30.0).exp();
            phase += freq / RATE as f32;
            (phase * TAU).sin() * envelope(t, 0.003, 0.04) * 0.4
        })
        .collect()
}

/// Ein trockenes Klicken für Knöpfe.
fn click() -> Vec<f32> {
    let mut noise = Noise(3);
    (0..samples(0.04))
        .map(|i| {
            let t = time(i);
            ((t * 1800.0 * TAU).sin() * 0.6 + noise.next() * 0.4) * envelope(t, 0.001, 0.008) * 0.35
        })
        .collect()
}

/// Zwei absteigende, traurige Töne.
fn sad() -> Vec<f32> {
    melody(&[(392.0, 0.5), (311.1, 0.9)], 0.28, 0.25)
}

fn normalize(mut sound: Vec<f32>) -> Vec<f32> {
    let peak = sound.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak > 0.98 {
        for s in &mut sound {
            *s *= 0.98 / peak;
        }
    }
    sound
}

fn write_wav(path: &Path, sound: &[f32]) {
    let data: Vec<u8> = sound
        .iter()
        .flat_map(|s| ((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes())
        .collect();
    let mut wav = Vec::with_capacity(44 + data.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // Mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    fs::write(path, wav).expect("WAV-Datei schreiben");
}

fn main() {
    let folder = Path::new("assets/sounds");
    fs::create_dir_all(folder).expect("Ordner assets/sounds anlegen");

    let mut alarm = Vec::new();
    for (k, at) in [0.0, 0.18, 0.36].into_iter().enumerate() {
        mix_into(&mut alarm, &thump(0.8, 11 + k as u64), at);
    }
    // C-Dur aufwärts für Geburt und Forschung, eine kleine Fanfare für den Sieg.
    let (c5, e5, g5, c6) = (523.3, 659.3, 784.0, 1046.5);
    let sounds: Vec<(&str, Vec<f32>)> = vec![
        ("stampfen", thump(0.9, 1)),
        ("alarm", alarm),
        ("graben", scratch(5)),
        ("bussard", buzzard_cry()),
        ("fuchs", rustle()),
        ("geburt", melody(&[(c5, 0.5), (e5, 0.5), (g5, 0.8)], 0.12, 0.22)),
        ("erforscht", melody(&[(g5, 0.4), (c6, 0.4), (e5 * 2.0, 0.4), (g5 * 2.0, 0.9)], 0.09, 0.18)),
        ("geschnappt", sad()),
        ("bauen", plop()),
        ("klick", click()),
        (
            "sieg",
            melody(&[(c5, 0.3), (c5, 0.3), (c5, 0.3), (e5, 0.5), (g5, 0.3), (e5, 0.3), (c6, 1.4)], 0.16, 0.22),
        ),
        ("ende", melody(&[(392.0, 0.6), (349.2, 0.6), (311.1, 0.6), (261.6, 1.6)], 0.4, 0.22)),
    ];
    for (name, sound) in sounds {
        let path = folder.join(format!("{name}.wav"));
        write_wav(&path, &normalize(sound));
        println!("{}", path.display());
    }
}
