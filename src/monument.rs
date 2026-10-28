//! Denkmäler: besondere Bauten, die sich die Familie mit Errungenschaften verdient. Jede
//! Errungenschaft schaltet ein Denkmal frei, das man kostenlos und einmal aufstellen kann.
//! Sie sind vor allem Schmuck und stehen wie Bäume auf der Wiese; jedes hat eine kleine Wirkung.
//!
//! Jede Errungenschaft hat drei Stufen mit immer höheren Zielen. Das Denkmal bleibt, wo es steht,
//! und wird mit jeder Stufe prächtiger: erst schlicht, dann geschmückt, zuletzt mit Gold.
//!
//! Wird eine Stufe erreicht, gibt es eine Fanfare: ein Banner mit dem Denkmal, Konfetti über den
//! ganzen Bildschirm und eine Nachricht. Beim Aufstellen und bei jeder neuen Stufe fliegt Konfetti
//! über dem Denkmal.

use std::collections::HashMap;

use bevy::picking::Pickable;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::audio::{Sound, Sounds};
use crate::cat::CatBond;
use crate::colony::{Messages, Rng};
use crate::daynight::DayClock;
use crate::effects::{DirtAssets, throw_confetti};
use crate::ground::to_world3;
use crate::map::{Depth, Feature, Grid, TILE_SIZE, footprint_middle};
use crate::progress::{Progress, Tech};
use crate::rabbit::Rabbit;

pub struct MonumentPlugin;

impl Plugin for MonumentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Standing>()
            .init_resource::<Fanfare>()
            .init_resource::<Unveil>()
            .add_systems(Startup, spawn_banner)
            .add_systems(
                Update,
                (find_standing, check_feats, count_festival_nights, play_fanfare, fly_confetti, unveil),
            );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Monument {
    Statue,
    Shovel,
    FoxStone,
    FeatherMast,
    SnowRabbit,
    GiantCarrot,
    AncestorStone,
    CrystalObelisk,
    CatBasket,
    Maypole,
    MoleMonument,
    BuilderArch,
    GoldenClover,
    AlarmBell,
}

/// So viele Stufen hat jede Errungenschaft.
pub const TIERS: u8 = 3;

/// Wirkungen, bewusst klein: Denkmäler sind vor allem Schmuck.
pub const STATUE_MOOD: f32 = 0.05;
pub const CLOVER_MOOD: f32 = 0.03;
/// So viel der Grabzeit bleibt mit Schaufelmonument.
pub const SHOVEL_DIG_FACTOR: f32 = 0.9;
/// So viel schneller geht es durch den Bau mit Baumeister-Bogen.
pub const ARCH_SPEED: f32 = 1.1;
/// So viel mehr Erde bringt das Graben mit Kristallobelisk.
pub const CRYSTAL_EARTH: f32 = 1.1;
/// So viel schneller wachsen Junge mit Ahnenstein.
pub const ANCESTOR_GROWTH: f32 = 1.1;
/// So viel des Winterhungers bleibt mit Winterkaninchen.
pub const SNOW_HUNGER: f32 = 0.9;
/// So viel schneller wächst das Gras bei der Riesenmöhre.
pub const CARROT_GROWTH: f32 = 1.25;
/// So viel weiter sehen Wachen beim Federmast nach dem Bussard.
pub const MAST_SIGHT: f32 = 1.2;
/// So viel der Wartezeit auf den nächsten Maulwurfshügel bleibt mit Maulwurfsdenkmal.
pub const MOLE_WAIT: f32 = 0.75;
/// So viel schneller flüchten alle mit Alarmglocke.
pub const BELL_FLEE: f32 = 1.1;

/// Die drei Bilder eines Denkmals, eins pro Stufe.
macro_rules! tiers {
    ($name:literal) => {
        [
            concat!("denkmal_", $name, "_1.png"),
            concat!("denkmal_", $name, "_2.png"),
            concat!("denkmal_", $name, "_3.png"),
        ]
    };
}

impl Monument {
    pub const ALL: [Monument; 14] = [
        Monument::Statue,
        Monument::Shovel,
        Monument::FoxStone,
        Monument::FeatherMast,
        Monument::SnowRabbit,
        Monument::GiantCarrot,
        Monument::AncestorStone,
        Monument::CrystalObelisk,
        Monument::CatBasket,
        Monument::Maypole,
        Monument::MoleMonument,
        Monument::BuilderArch,
        Monument::GoldenClover,
        Monument::AlarmBell,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Monument::Statue => "Kaninchenstatue",
            Monument::Shovel => "Schaufelmonument",
            Monument::FoxStone => "Fuchsschreck",
            Monument::FeatherMast => "Federmast",
            Monument::SnowRabbit => "Winterkaninchen",
            Monument::GiantCarrot => "Riesenmöhre",
            Monument::AncestorStone => "Ahnenstein",
            Monument::CrystalObelisk => "Kristallobelisk",
            Monument::CatBasket => "Katzenkorb",
            Monument::Maypole => "Maibaum",
            Monument::MoleMonument => "Maulwurfsdenkmal",
            Monument::BuilderArch => "Baumeister-Bogen",
            Monument::GoldenClover => "Goldenes Kleeblatt",
            Monument::AlarmBell => "Alarmglocke",
        }
    }

    /// Die Bilder unter `assets/einrichtung`, für Stufe 1 bis 3.
    pub fn images(self) -> [&'static str; 3] {
        match self {
            Monument::Statue => tiers!("statue"),
            Monument::Shovel => tiers!("schaufel"),
            Monument::FoxStone => tiers!("fuchsschreck"),
            Monument::FeatherMast => tiers!("federmast"),
            Monument::SnowRabbit => tiers!("winterkaninchen"),
            Monument::GiantCarrot => tiers!("riesenmoehre"),
            Monument::AncestorStone => tiers!("ahnenstein"),
            Monument::CrystalObelisk => tiers!("obelisk"),
            Monument::CatBasket => tiers!("katzenkorb"),
            Monument::Maypole => tiers!("maibaum"),
            Monument::MoleMonument => tiers!("maulwurf"),
            Monument::BuilderArch => tiers!("bogen"),
            Monument::GoldenClover => tiers!("kleeblatt"),
            Monument::AlarmBell => tiers!("glocke"),
        }
    }

    /// Das Bild für diese Stufe; noch nicht verdiente Denkmäler zeigen die erste.
    pub fn image(self, tier: u8) -> &'static str {
        self.images()[usize::from(tier.clamp(1, TIERS) - 1)]
    }

    /// Bildgröße in Bildpixeln, wie in `art_src/einrichtung` gezeichnet; gleich für alle Stufen.
    pub fn picture_size(self) -> Vec2 {
        match self {
            Monument::Statue => Vec2::new(240.0, 340.0),
            Monument::Shovel => Vec2::new(128.0, 184.0),
            Monument::FoxStone => Vec2::new(260.0, 210.0),
            Monument::FeatherMast => Vec2::new(104.0, 260.0),
            Monument::SnowRabbit => Vec2::new(132.0, 168.0),
            Monument::GiantCarrot => Vec2::new(220.0, 320.0),
            Monument::AncestorStone => Vec2::new(128.0, 168.0),
            Monument::CrystalObelisk => Vec2::new(200.0, 360.0),
            Monument::CatBasket => Vec2::new(152.0, 112.0),
            Monument::Maypole => Vec2::new(250.0, 420.0),
            Monument::MoleMonument => Vec2::new(128.0, 152.0),
            Monument::BuilderArch => Vec2::new(288.0, 248.0),
            Monument::GoldenClover => Vec2::new(220.0, 320.0),
            Monument::AlarmBell => Vec2::new(128.0, 216.0),
        }
    }

    /// Seitenlänge in Feldern: große Denkmäler belegen 2 × 2.
    pub fn size(self) -> i32 {
        match self {
            Monument::Statue
            | Monument::FoxStone
            | Monument::GiantCarrot
            | Monument::CrystalObelisk
            | Monument::Maypole
            | Monument::BuilderArch
            | Monument::GoldenClover => 2,
            _ => 1,
        }
    }

    /// So viel braucht es für Stufe 1, 2 und 3.
    fn goals(self) -> [f32; 3] {
        match self {
            Monument::Statue => [100.0, 250.0, 450.0],
            Monument::Shovel => [100.0, 500.0, 1500.0],
            Monument::FoxStone => [10.0, 50.0, 200.0],
            Monument::FeatherMast => [10.0, 40.0, 100.0],
            Monument::SnowRabbit => [5.0, 10.0, 20.0],
            Monument::GiantCarrot => [1000.0, 10000.0, 40000.0],
            Monument::AncestorStone => [5.0, 12.0, 25.0],
            Monument::CrystalObelisk => [1.0, 30.0, 100.0],
            // Erst Freundschaft, dann verjagte Wühlmäuse obendrauf.
            Monument::CatBasket => [1.0, 6.0, 16.0],
            Monument::Maypole => [10.0, 30.0, 75.0],
            Monument::MoleMonument => [20.0, 100.0, 300.0],
            Monument::BuilderArch => [150.0, 350.0, 600.0],
            Monument::GoldenClover => {
                let all = Tech::ALL.len() as f32;
                [(all / 3.0).ceil(), (all * 2.0 / 3.0).ceil(), all]
            }
            Monument::AlarmBell => [15.0, 60.0, 150.0],
        }
    }

    /// So viel braucht es für diese Stufe (1 bis 3).
    pub fn goal(self, tier: u8) -> f32 {
        self.goals()[usize::from(tier.clamp(1, TIERS) - 1)]
    }

    /// Was die Familie für diese Stufe schaffen muss.
    pub fn condition(self, tier: u8) -> String {
        let goal = self.goal(tier);
        let n = number(goal);
        match self {
            Monument::Statue => format!("{n} Kaninchen auf einmal in der Familie"),
            Monument::Shovel => format!("{n} Erde ausgegraben"),
            Monument::FoxStone => format!("{n} Füchse ziehen ohne Beute ab"),
            Monument::FeatherMast => format!("Die Wachen warnen {n}-mal vor dem Bussard"),
            Monument::SnowRabbit => format!("{n} Winter überstanden"),
            Monument::GiantCarrot => format!("{n} Futter eingelagert"),
            Monument::AncestorStone => format!("Ein Kaninchen der {n}. Generation"),
            Monument::CrystalObelisk if goal <= 1.0 => "Ein Gang auf der tiefsten Ebene".into(),
            Monument::CrystalObelisk => format!("{n} Gangfelder auf der tiefsten Ebene"),
            Monument::CatBasket if goal <= 1.0 => "Minka ist eure Freundin".into(),
            Monument::CatBasket => format!("Minka verjagt {}-mal Wühlmäuse vom Vorrat", number(goal - 1.0)),
            Monument::Maypole => format!("{n} Festnächte: Nächte mit Festplatz"),
            Monument::MoleMonument => format!("{n} Maulwurfshügel eingeebnet"),
            Monument::BuilderArch => format!("{n} Gangfelder gegraben"),
            Monument::GoldenClover if tier >= TIERS => "Alles im Forschungsbaum erforscht".into(),
            Monument::GoldenClover => format!("{n} Dinge im Forschungsbaum erforscht"),
            Monument::AlarmBell => format!("{n} Schutzbauten: Hecken, Fluchtlöcher, Wachen, Pfropfen, Dämme"),
        }
    }

    /// Die kleine Wirkung, für Tooltip und Übersicht.
    pub fn effect(self) -> &'static str {
        match self {
            Monument::Statue => "Kaninchen in der Nähe sind etwas zufriedener.",
            Monument::Shovel => "Alle graben ein wenig schneller.",
            Monument::FoxStone => "Füchse trauen sich nicht nah heran: Wer im Kreis hockt, wird nicht gejagt.",
            Monument::FeatherMast => "Wachen in der Nähe sehen den Bussard früher.",
            Monument::SnowRabbit => "Im Winter wird die Familie etwas weniger hungrig.",
            Monument::GiantCarrot => "Gras wächst im Umkreis schneller nach.",
            Monument::AncestorStone => "Junge wachsen ein wenig schneller heran.",
            Monument::CrystalObelisk => "Graben bringt etwas mehr Erde.",
            Monument::CatBasket => "Minka döst tagsüber in ihrem Korb.",
            Monument::Maypole => "Mit Festplatz hält die Familie eine Nacht länger ohne Nest aus.",
            Monument::MoleMonument => "Maulwürfe werfen öfter Hügel auf.",
            Monument::BuilderArch => "Im Bau hoppeln alle etwas schneller.",
            Monument::GoldenClover => "Die ganze Familie ist ein wenig zufriedener.",
            Monument::AlarmBell => "Bei Gefahr flüchten alle etwas schneller.",
        }
    }

    /// Wie weit die Wirkung reicht, in Weltmaß; `None` = überall.
    pub fn reach(self) -> Option<f32> {
        match self {
            Monument::Statue | Monument::FeatherMast => Some(3.0 * TILE_SIZE),
            Monument::FoxStone | Monument::GiantCarrot => Some(2.5 * TILE_SIZE),
            _ => None,
        }
    }
}

/// Große Zahlen mit Leerzeichen in Dreiergruppen: „10 000“.
pub fn number(value: f32) -> String {
    let digits = format!("{:.0}", value.max(0.0));
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) && digits.len() > 4 {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

/// Sterne für die Stufe: „★★☆“.
pub fn stars(tier: u8) -> String {
    (1..=TIERS).map(|t| if t <= tier { '★' } else { '☆' }).collect()
}

/// Zähler für die Errungenschaften und was schon verdient ist; Teil des Fortschritts.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Feats {
    pub earth_dug: f32,
    pub food_stored: f32,
    pub foxes_repelled: u32,
    pub buzzard_warnings: u32,
    pub molehills: u32,
    pub festival_nights: u32,
    /// Die höchste Generation, die es gab; die Gründer sind die erste.
    pub generation: u32,
    /// Wie oft Minka als Freundin Wühlmäuse vom Vorrat verjagt hat.
    pub voles_chased: u32,
    /// Gebaute Schutzbauten: Hecken, Fluchtlöcher, Wachposten, Nachtwachen, Erdpfropfen, Dämme.
    pub safety_built: u32,
    /// Jede erreichte Stufe ein Eintrag, in der Reihenfolge, in der sie verdient wurden: Steht ein
    /// Denkmal zweimal darin, hat es Stufe 2.
    pub earned: Vec<Monument>,
}

impl Feats {
    pub fn has(&self, monument: Monument) -> bool {
        self.earned.contains(&monument)
    }

    /// Die erreichte Stufe: 0 = noch nicht verdient.
    pub fn tier(&self, monument: Monument) -> u8 {
        self.earned.iter().filter(|m| **m == monument).count().min(usize::from(TIERS)) as u8
    }

    /// Alle erreichten Stufen zusammen; ändert sich nur, wenn eine neue dazukommt.
    pub fn tiers_earned(&self) -> usize {
        self.earned.len()
    }
}

/// Was sich nicht mitzählen lässt, sondern jedes Mal nachgesehen wird.
pub struct Tally {
    pub rabbits: usize,
    pub dug: usize,
    pub cat_friend: bool,
    /// Gangfelder auf der tiefsten Ebene.
    pub deep: usize,
}

impl Tally {
    pub fn of(grid: &Grid, rabbits: usize, cat: &CatBond) -> Self {
        let dug: Vec<Depth> = Grid::positions().filter_map(|p| grid.under(p).dug).collect();
        Self {
            rabbits,
            dug: dug.len(),
            deep: dug.iter().filter(|d| **d == Depth::Deep).count(),
            cat_friend: cat.friend,
        }
    }
}

/// Wie weit die Familie bei dieser Errungenschaft ist, im Maß von `goal`.
pub fn score(monument: Monument, progress: &Progress, tally: &Tally) -> f32 {
    let feats = &progress.feats;
    match monument {
        Monument::Statue => tally.rabbits as f32,
        Monument::Shovel => feats.earth_dug,
        Monument::FoxStone => feats.foxes_repelled as f32,
        Monument::FeatherMast => feats.buzzard_warnings as f32,
        Monument::SnowRabbit => progress.winters_survived as f32,
        Monument::GiantCarrot => feats.food_stored,
        Monument::AncestorStone => feats.generation as f32,
        Monument::CrystalObelisk => tally.deep as f32,
        Monument::CatBasket if tally.cat_friend => 1.0 + feats.voles_chased as f32,
        Monument::CatBasket => 0.0,
        Monument::Maypole => feats.festival_nights as f32,
        Monument::MoleMonument => feats.molehills as f32,
        Monument::BuilderArch => tally.dug as f32,
        Monument::GoldenClover => progress.researched() as f32,
        Monument::AlarmBell => feats.safety_built as f32,
    }
}

/// Wo welches Denkmal steht: die Mitte seiner Fläche in Weltmaß.
#[derive(Resource, Default)]
pub struct Standing(pub HashMap<Monument, Vec2>);

impl Standing {
    pub fn has(&self, monument: Monument) -> bool {
        self.0.contains_key(&monument)
    }

    /// Ob `at` im Wirkungskreis dieses Denkmals liegt.
    pub fn near(&self, monument: Monument, at: Vec2) -> bool {
        let reach = monument.reach().unwrap_or(0.0);
        self.0.get(&monument).is_some_and(|middle| middle.distance(at) < reach)
    }

    pub fn of(grid: &Grid) -> HashMap<Monument, Vec2> {
        Grid::positions()
            .filter_map(|p| {
                let tile = grid.surface(p);
                match tile.feature {
                    Some(Feature::Monument(m)) if tile.part_of == Some(p) => Some((m, footprint_middle(p, m.size()))),
                    _ => None,
                }
            })
            .collect()
    }
}

fn find_standing(grid: Res<Grid>, mut standing: ResMut<Standing>) {
    if !grid.is_changed() {
        return;
    }
    let now = Standing::of(&grid);
    if standing.0 != now {
        standing.0 = now;
    }
}

fn check_feats(
    time: Res<Time>,
    grid: Res<Grid>,
    cat: Res<CatBond>,
    rabbits: Query<&Rabbit>,
    mut progress: ResMut<Progress>,
    mut fanfare: ResMut<Fanfare>,
    mut messages: ResMut<Messages>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer < 1.0 {
        return;
    }
    *timer = 0.0;
    let latest = rabbits.iter().map(|r| r.generation).max().unwrap_or(0);
    if latest > progress.feats.generation {
        progress.feats.generation = latest;
    }
    let tally = Tally::of(&grid, rabbits.iter().count(), &cat);
    for monument in Monument::ALL {
        let tier = progress.feats.tier(monument);
        if tier >= TIERS || score(monument, &progress, &tally) < monument.goal(tier + 1) {
            continue;
        }
        let tier = tier + 1;
        progress.feats.earned.push(monument);
        fanfare.queue.push((monument, tier));
        let reward = if tier == 1 {
            format!("Neues Denkmal: {}", monument.label())
        } else {
            format!("{} wird prächtiger, Stufe {tier}", monument.label())
        };
        messages.push(format!("Errungenschaft: {}! {reward}.", monument.condition(tier)));
    }
}

/// Eine Festnacht: Bricht die Nacht herein und es gibt einen Festplatz, wird gefeiert.
fn count_festival_nights(clock: Res<DayClock>, grid: Res<Grid>, mut progress: ResMut<Progress>, mut was_night: Local<bool>) {
    let night = clock.is_night();
    if night && !*was_night && Grid::positions().any(|p| grid.surface(p).feature == Some(Feature::Festival)) {
        progress.feats.festival_nights += 1;
    }
    *was_night = night;
}

// --- Die Fanfare beim Freischalten ---

/// So lange steht das Banner da, in echten Sekunden.
const BANNER_SECONDS: f32 = 7.0;
const CONFETTI_PIECES: usize = 160;
const GOLD: Color = Color::srgb(1.0, 0.84, 0.36);
const CONFETTI_COLORS: [Color; 7] = [
    Color::srgb(0.85, 0.31, 0.23),
    Color::srgb(0.94, 0.75, 0.25),
    Color::srgb(0.35, 0.60, 0.28),
    Color::srgb(0.35, 0.54, 0.82),
    Color::srgb(0.85, 0.42, 0.56),
    Color::srgb(0.94, 0.54, 0.23),
    Color::srgb(0.98, 0.96, 0.90),
];

/// Erreichte Stufen, die noch ihre Fanfare bekommen, und die gerade gezeigte.
#[derive(Resource, Default)]
pub struct Fanfare {
    pub queue: Vec<(Monument, u8)>,
    showing: Option<f32>,
}

#[derive(Component)]
struct Banner;

#[derive(Component)]
struct BannerImage;

#[derive(Component)]
struct BannerName;

#[derive(Component)]
struct BannerText;

#[derive(Component)]
struct ConfettiPiece {
    position: Vec2,
    velocity: Vec2,
    size: Vec2,
    spin: f32,
    phase: f32,
}

fn text(size: f32, color: Color) -> impl Bundle {
    (
        TextColor(color),
        TextFont {
            font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
            ..TextFont::from_font_size(size)
        },
        TextLayout::justify(Justify::Center),
        Pickable::IGNORE,
    )
}

fn spawn_banner(mut commands: Commands) {
    let row = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(70.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(20),
            Pickable::IGNORE,
        ))
        .id();
    let banner = commands
        .spawn((
            Banner,
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(36.0), Val::Px(18.0)),
                border: UiRect::all(Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                max_width: Val::Px(560.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.24, 0.16, 0.09, 0.94)),
            BorderColor::all(GOLD),
            UiTransform::default(),
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();
    commands.spawn((Text::new("Errungenschaft!"), text(30.0, GOLD), ChildOf(banner)));
    commands.spawn((
        BannerImage,
        ImageNode::default(),
        Node {
            height: Val::Px(150.0),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(banner),
    ));
    commands.spawn((BannerName, Text::default(), text(24.0, Color::srgb(0.98, 0.95, 0.88)), ChildOf(banner)));
    commands.spawn((BannerText, Text::default(), text(16.0, Color::srgb(0.85, 0.80, 0.70)), ChildOf(banner)));
}

fn play_fanfare(
    mut commands: Commands,
    time: Res<Time<Real>>,
    asset_server: Res<AssetServer>,
    windows: Query<&Window>,
    standing: Res<Standing>,
    mut fanfare: ResMut<Fanfare>,
    mut unveil: ResMut<Unveil>,
    mut sounds: ResMut<Sounds>,
    mut rng: ResMut<Rng>,
    mut banner: Single<(&mut Node, &mut UiTransform), With<Banner>>,
    mut image: Single<&mut ImageNode, With<BannerImage>>,
    mut name: Single<&mut Text, (With<BannerName>, Without<BannerText>)>,
    mut info: Single<&mut Text, (With<BannerText>, Without<BannerName>)>,
) {
    let (node, transform) = &mut *banner;
    if fanfare.showing.is_none() {
        if fanfare.queue.is_empty() {
            return;
        }
        let (monument, tier) = fanfare.queue.remove(0);
        fanfare.showing = Some(0.0);
        image.image = asset_server.load(format!("einrichtung/{}", monument.image(tier)));
        name.0 = format!("{}  {}", monument.label(), stars(tier));
        let size = if monument.size() > 1 { "2 × 2 Felder" } else { "1 Feld" };
        let next = if tier < TIERS {
            format!("\nNächste Stufe: {}.", monument.condition(tier + 1))
        } else {
            "\nDie höchste Stufe!".into()
        };
        let place = match standing.0.get(&monument) {
            Some(middle) => {
                // Steht es schon, schmückt es sich gleich dort, wo es steht.
                unveil.0.push((monument, *middle, Celebrate::Upgraded));
                "Das Denkmal schmückt sich gleich dort, wo es steht.".to_string()
            }
            None => format!("Steht kostenlos unter „Denkmäler“ bereit ({size})."),
        };
        info.0 = format!("{}\n{}\n{place}{next}", monument.condition(tier), monument.effect());
        node.display = Display::Flex;
        sounds.play(Sound::Victory);
        let width = windows.iter().next().map_or(1280.0, |w| w.width());
        for _ in 0..CONFETTI_PIECES {
            spawn_confetti(&mut commands, &mut rng, width);
        }
    }
    let Some(shown) = fanfare.showing.as_mut() else {
        return;
    };
    *shown += time.delta_secs();
    // Springt auf, federt kurz nach und schrumpft am Ende wieder weg.
    let t = *shown;
    let grow = (t / 0.45).min(1.0);
    let pop = 1.0 + 0.12 * (grow * std::f32::consts::PI).sin();
    let shrink = ((BANNER_SECONDS - t) / 0.35).clamp(0.0, 1.0);
    transform.scale = Vec2::splat((0.5 + 0.5 * grow) * pop * shrink);
    if t >= BANNER_SECONDS {
        node.display = Display::None;
        fanfare.showing = None;
    }
}

fn spawn_confetti(commands: &mut Commands, rng: &mut Rng, width: f32) {
    let position = Vec2::new(rng.range(0.0, width), rng.range(-260.0, -10.0));
    let size = Vec2::new(rng.range(7.0, 12.0), rng.range(10.0, 16.0));
    commands.spawn((
        ConfettiPiece {
            position,
            velocity: Vec2::new(rng.range(-40.0, 40.0), rng.range(80.0, 170.0)),
            size,
            spin: rng.range(4.0, 9.0),
            phase: rng.range(0.0, 6.3),
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(position.x),
            top: Val::Px(position.y),
            width: Val::Px(size.x),
            height: Val::Px(size.y),
            border_radius: BorderRadius::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(CONFETTI_COLORS[rng.below(CONFETTI_COLORS.len())]),
        // Hinter dem Banner, damit man seinen Text lesen kann.
        GlobalZIndex(15),
        Pickable::IGNORE,
    ));
}

/// Konfetti flattert herab: Es schwingt hin und her und dreht sich, dann ist es weg.
fn fly_confetti(
    mut commands: Commands,
    time: Res<Time<Real>>,
    windows: Query<&Window>,
    mut pieces: Query<(Entity, &mut ConfettiPiece, &mut Node)>,
) {
    let dt = time.delta_secs();
    let bottom = windows.iter().next().map_or(800.0, |w| w.height()) + 20.0;
    for (entity, mut piece, mut node) in &mut pieces {
        piece.phase += piece.spin * dt;
        let sway = piece.phase.sin() * 50.0;
        let velocity = piece.velocity;
        piece.position += Vec2::new(velocity.x + sway, velocity.y) * dt;
        if piece.position.y > bottom {
            commands.entity(entity).despawn();
            continue;
        }
        // Das Umdrehen sieht man daran, dass es mal schmal, mal breit ist.
        let turn = piece.phase.cos().abs().max(0.15);
        node.left = Val::Px(piece.position.x);
        node.top = Val::Px(piece.position.y);
        node.width = Val::Px(piece.size.x * turn);
        node.height = Val::Px(piece.size.y);
    }
}

// --- Einweihung beim Aufstellen ---

/// Warum über einem Denkmal Konfetti fliegt.
pub enum Celebrate {
    Placed,
    Upgraded,
}

/// Denkmäler, über denen Konfetti fliegen soll, mit der Mitte ihrer Fläche in Weltmaß.
#[derive(Resource, Default)]
pub struct Unveil(pub Vec<(Monument, Vec2, Celebrate)>);

fn unveil(
    mut commands: Commands,
    mut unveiled: ResMut<Unveil>,
    assets: Res<DirtAssets>,
    mut rng: ResMut<Rng>,
    mut sounds: ResMut<Sounds>,
    mut messages: ResMut<Messages>,
) {
    for (monument, middle, why) in unveiled.0.drain(..) {
        let height = if monument.size() > 1 { 110.0 } else { 60.0 };
        for _ in 0..70 {
            throw_confetti(&mut commands, &assets, &mut rng, to_world3(middle, height));
        }
        // Bei einer neuen Stufe spielt schon die Fanfare und es gibt schon eine Nachricht.
        if let Celebrate::Placed = why {
            sounds.play(Sound::Victory);
            messages.push(format!("Die Familie weiht {} ein! {}", monument.label(), monument.effect()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_monument_has_its_own_images() {
        let mut images: Vec<&str> = Monument::ALL.iter().flat_map(|m| m.images()).collect();
        images.sort();
        images.dedup();
        assert_eq!(images.len(), Monument::ALL.len() * usize::from(TIERS));
    }

    #[test]
    fn a_feat_counts_once_its_goal_is_reached() {
        let mut progress = Progress::default();
        let tally = Tally { rabbits: 12, dug: 30, deep: 0, cat_friend: true };
        assert_eq!(score(Monument::CatBasket, &progress, &tally), Monument::CatBasket.goal(1));
        assert!(score(Monument::Shovel, &progress, &tally) < Monument::Shovel.goal(1));
        progress.feats.earth_dug = 100.0;
        assert!(score(Monument::Shovel, &progress, &tally) >= Monument::Shovel.goal(1));
    }

    #[test]
    fn each_tier_asks_for_more_and_shows_in_the_earned_list() {
        for monument in Monument::ALL {
            assert!(monument.goal(1) < monument.goal(2) && monument.goal(2) < monument.goal(3), "{monument:?}");
        }
        let mut feats = Feats::default();
        assert_eq!(feats.tier(Monument::Statue), 0);
        feats.earned = vec![Monument::Statue, Monument::Shovel, Monument::Statue];
        assert_eq!(feats.tier(Monument::Statue), 2);
        assert_eq!(feats.tier(Monument::Shovel), 1);
        assert_eq!(number(40000.0), "40\u{202f}000");
        assert_eq!(number(1500.0), "1500");
        assert_eq!(stars(2), "★★☆");
    }
}
