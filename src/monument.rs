//! Denkmäler: besondere Bauten, die sich die Familie mit Errungenschaften verdient. Jede
//! Errungenschaft schaltet ein Denkmal frei, das man kostenlos und einmal aufstellen kann.
//! Sie sind vor allem Schmuck und stehen wie Bäume auf der Wiese; jedes hat eine kleine Wirkung.
//!
//! Wird eine Errungenschaft erreicht, gibt es eine Fanfare: ein Banner mit dem Denkmal, Konfetti
//! über den ganzen Bildschirm und eine Nachricht. Beim Aufstellen fliegt Konfetti über dem Denkmal.

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
use crate::map::{Depth, Feature, Grid, TILE_SIZE, TilePos, footprint_middle};
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
}

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

impl Monument {
    pub const ALL: [Monument; 13] = [
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
        }
    }

    /// Das Bild unter `assets/einrichtung`.
    pub fn image(self) -> &'static str {
        match self {
            Monument::Statue => "denkmal_statue.png",
            Monument::Shovel => "denkmal_schaufel.png",
            Monument::FoxStone => "denkmal_fuchsschreck.png",
            Monument::FeatherMast => "denkmal_federmast.png",
            Monument::SnowRabbit => "denkmal_winterkaninchen.png",
            Monument::GiantCarrot => "denkmal_riesenmoehre.png",
            Monument::AncestorStone => "denkmal_ahnenstein.png",
            Monument::CrystalObelisk => "denkmal_obelisk.png",
            Monument::CatBasket => "denkmal_katzenkorb.png",
            Monument::Maypole => "denkmal_maibaum.png",
            Monument::MoleMonument => "denkmal_maulwurf.png",
            Monument::BuilderArch => "denkmal_bogen.png",
            Monument::GoldenClover => "denkmal_kleeblatt.png",
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

    /// So viel braucht es für die Errungenschaft.
    pub fn goal(self) -> f32 {
        match self {
            Monument::Statue => 300.0,
            Monument::Shovel => 100.0,
            Monument::FoxStone => 10.0,
            Monument::FeatherMast => 15.0,
            Monument::SnowRabbit => 3.0,
            Monument::GiantCarrot => 1000.0,
            Monument::AncestorStone => 5.0,
            Monument::CrystalObelisk | Monument::CatBasket => 1.0,
            Monument::Maypole => 10.0,
            Monument::MoleMonument => 20.0,
            Monument::BuilderArch => 200.0,
            Monument::GoldenClover => Tech::ALL.len() as f32,
        }
    }

    /// Was die Familie dafür schaffen muss.
    pub fn condition(self) -> &'static str {
        match self {
            Monument::Statue => "300 Kaninchen auf einmal in der Familie",
            Monument::Shovel => "100 Erde ausgegraben",
            Monument::FoxStone => "10 Füchse ziehen ohne Beute ab",
            Monument::FeatherMast => "Die Wachen warnen 15-mal vor dem Bussard",
            Monument::SnowRabbit => "3 Winter überstanden",
            Monument::GiantCarrot => "1000 Futter eingelagert",
            Monument::AncestorStone => "Ein Kaninchen der 5. Generation",
            Monument::CrystalObelisk => "Ein Gang auf der tiefsten Ebene",
            Monument::CatBasket => "Minka ist eure Freundin",
            Monument::Maypole => "10 Festnächte: Nächte mit Festplatz",
            Monument::MoleMonument => "20 Maulwurfshügel eingeebnet",
            Monument::BuilderArch => "200 Gangfelder gegraben",
            Monument::GoldenClover => "Alles im Forschungsbaum erforscht",
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

/// Zähler für die Errungenschaften und was schon verdient ist; Teil des Fortschritts.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Feats {
    pub earth_dug: f32,
    pub food_stored: f32,
    pub foxes_repelled: u32,
    pub buzzard_warnings: u32,
    pub molehills: u32,
    pub festival_nights: u32,
    /// Die höchste Generation, die es gab; die Gründer sind die erste.
    pub generation: u32,
    /// In der Reihenfolge, in der sie verdient wurden.
    pub earned: Vec<Monument>,
}

impl Feats {
    pub fn has(&self, monument: Monument) -> bool {
        self.earned.contains(&monument)
    }
}

/// Was sich nicht mitzählen lässt, sondern jedes Mal nachgesehen wird.
pub struct Tally {
    pub rabbits: usize,
    pub dug: usize,
    pub deep: bool,
    pub cat_friend: bool,
}

impl Tally {
    pub fn of(grid: &Grid, rabbits: usize, cat: &CatBond) -> Self {
        let dug: Vec<Depth> = Grid::positions().filter_map(|p| grid.under(p).dug).collect();
        Self {
            rabbits,
            dug: dug.len(),
            deep: dug.contains(&Depth::Deep),
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
        Monument::CrystalObelisk => f32::from(u8::from(tally.deep)),
        Monument::CatBasket => f32::from(u8::from(tally.cat_friend)),
        Monument::Maypole => feats.festival_nights as f32,
        Monument::MoleMonument => feats.molehills as f32,
        Monument::BuilderArch => tally.dug as f32,
        Monument::GoldenClover => progress.researched() as f32,
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
        if progress.feats.has(monument) || score(monument, &progress, &tally) < monument.goal() {
            continue;
        }
        progress.feats.earned.push(monument);
        fanfare.queue.push(monument);
        messages.push(format!("Errungenschaft: {}! Neues Denkmal: {}.", monument.condition(), monument.label()));
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

/// Freigeschaltete Denkmäler, die noch ihre Fanfare bekommen, und das gerade gezeigte.
#[derive(Resource, Default)]
pub struct Fanfare {
    pub queue: Vec<Monument>,
    showing: Option<(Monument, f32)>,
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
    mut fanfare: ResMut<Fanfare>,
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
        let monument = fanfare.queue.remove(0);
        fanfare.showing = Some((monument, 0.0));
        image.image = asset_server.load(format!("einrichtung/{}", monument.image()));
        name.0 = monument.label().to_string();
        let size = if monument.size() > 1 { "2 × 2 Felder" } else { "1 Feld" };
        info.0 = format!(
            "{}\n{}\nSteht jetzt kostenlos unter „Denkmäler“ bereit ({size}).",
            monument.condition(),
            monument.effect()
        );
        node.display = Display::Flex;
        sounds.play(Sound::Victory);
        let width = windows.iter().next().map_or(1280.0, |w| w.width());
        for _ in 0..CONFETTI_PIECES {
            spawn_confetti(&mut commands, &mut rng, width);
        }
    }
    let Some((_, shown)) = fanfare.showing.as_mut() else {
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

/// Gerade aufgestellte Denkmäler, über denen Konfetti fliegen soll.
#[derive(Resource, Default)]
pub struct Unveil(pub Vec<(Monument, TilePos)>);

fn unveil(
    mut commands: Commands,
    mut unveiled: ResMut<Unveil>,
    assets: Res<DirtAssets>,
    mut rng: ResMut<Rng>,
    mut sounds: ResMut<Sounds>,
    mut messages: ResMut<Messages>,
) {
    for (monument, center) in unveiled.0.drain(..) {
        let middle = footprint_middle(center, monument.size());
        let height = if monument.size() > 1 { 110.0 } else { 60.0 };
        for _ in 0..70 {
            throw_confetti(&mut commands, &assets, &mut rng, to_world3(middle, height));
        }
        sounds.play(Sound::Victory);
        messages.push(format!("Die Familie weiht {} ein! {}", monument.label(), monument.effect()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_monument_has_its_own_image() {
        let mut images: Vec<&str> = Monument::ALL.iter().map(|m| m.image()).collect();
        images.sort();
        images.dedup();
        assert_eq!(images.len(), Monument::ALL.len());
    }

    #[test]
    fn a_feat_counts_once_its_goal_is_reached() {
        let mut progress = Progress::default();
        let tally = Tally { rabbits: 12, dug: 30, deep: false, cat_friend: true };
        assert_eq!(score(Monument::CatBasket, &progress, &tally), Monument::CatBasket.goal());
        assert!(score(Monument::Shovel, &progress, &tally) < Monument::Shovel.goal());
        progress.feats.earth_dug = 100.0;
        assert!(score(Monument::Shovel, &progress, &tally) >= Monument::Shovel.goal());
    }
}
