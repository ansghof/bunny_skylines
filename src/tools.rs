//! Bauwerkzeuge: Mit gedrückter linker Maustaste einen Bereich aufziehen.
//! Die Kaninchen arbeiten die Aufträge danach selbst ab; Bauten an der Oberfläche stehen sofort.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;

use crate::audio::{Sound, Sounds};
use crate::camera::CursorWorld;
use crate::colony::{Colony, Messages, cost};
use crate::daynight::DayClock;
use crate::ground::gizmo_rect;
use crate::map::{Chamber, Depth, Feature, Grid, Layer, Soil, SurfaceTile, TILE_SIZE, TilePos, ViewLayer, tiles_between};
use crate::progress::{Progress, Stage, Tech};
use crate::toolbar::Toolbar;

pub struct ToolsPlugin;

impl Plugin for ToolsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Tool { kind: ToolKind::Point, depth: Depth::Medium })
            .init_resource::<Drag>()
            .init_resource::<SelectionCost>()
            .add_systems(Update, (select_tool, apply_drag, draw_preview).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolKind {
    /// Nur anklicken und ansehen, nichts bauen.
    Point,
    Dig,
    Entrance,
    Chamber(Chamber),
    Post,
    Clover,
    Feature(Feature),
    /// Erdpfropf im Gang, gegen den Marder.
    Plug,
    /// Platz für eine Nachtwache im Bau.
    Watch,
    Remove,
}

/// Warum ein Werkzeug noch gesperrt ist.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lock {
    Tech(Tech),
    Stage(Stage),
}

/// Alle Werkzeuge in der Reihenfolge der Werkzeugleiste, mit Taste und Tastenname.
pub const TOOLS: [(ToolKind, KeyCode, &str); 17] = [
    (ToolKind::Point, KeyCode::Escape, "Esc"),
    (ToolKind::Dig, KeyCode::KeyG, "G"),
    (ToolKind::Entrance, KeyCode::KeyE, "E"),
    (ToolKind::Chamber(Chamber::Sleeping), KeyCode::KeyN, "N"),
    (ToolKind::Chamber(Chamber::Storage), KeyCode::KeyV, "V"),
    (ToolKind::Chamber(Chamber::Story), KeyCode::KeyZ, "Z"),
    (ToolKind::Chamber(Chamber::Nursery), KeyCode::KeyJ, "J"),
    (ToolKind::Post, KeyCode::KeyP, "P"),
    (ToolKind::Feature(Feature::Hedge), KeyCode::KeyH, "H"),
    (ToolKind::Feature(Feature::BoltHole), KeyCode::KeyL, "L"),
    (ToolKind::Feature(Feature::SunSpot), KeyCode::KeyO, "O"),
    (ToolKind::Clover, KeyCode::KeyK, "K"),
    (ToolKind::Feature(Feature::Herbs), KeyCode::KeyU, "U"),
    (ToolKind::Feature(Feature::Playground), KeyCode::KeyM, "M"),
    (ToolKind::Watch, KeyCode::KeyI, "I"),
    (ToolKind::Plug, KeyCode::KeyC, "C"),
    (ToolKind::Remove, KeyCode::KeyX, "X"),
];

impl ToolKind {
    pub fn label(self) -> &'static str {
        match self {
            ToolKind::Point => "Zeigen",
            ToolKind::Dig => "Graben",
            ToolKind::Entrance => "Eingang",
            ToolKind::Chamber(chamber) => chamber.label(),
            ToolKind::Post => "Wachposten",
            ToolKind::Clover => "Kleefeld säen",
            ToolKind::Feature(feature) => feature.label(),
            ToolKind::Plug => "Erdpfropf",
            ToolKind::Watch => "Nachtwache",
            ToolKind::Remove => "Entfernen",
        }
    }

    /// Ein Satz dazu, was das Werkzeug tut, für den Tooltip.
    pub fn description(self) -> &'static str {
        match self {
            ToolKind::Point => "Kaninchen anklicken und ansehen, ohne etwas zu bauen. Esc wechselt jederzeit hierher.",
            ToolKind::Dig => "Gänge und Kammern ausheben lassen. Bringt Erde.",
            ToolKind::Entrance => "Einen Eingang von der Wiese in den Bau graben lassen.",
            ToolKind::Chamber(Chamber::Sleeping) => "Gegrabene Felder zur Schlafkammer machen. Mehr Nester, mehr Nachwuchs.",
            ToolKind::Chamber(Chamber::Storage) => "Gegrabene Felder zur Vorratskammer machen. Hier lagert das Futter.",
            ToolKind::Chamber(Chamber::Story) => {
                "Hier hören Kaninchen Geschichten und sammeln Weisheit für den Forschungsbaum. Alte erzählen am besten."
            }
            ToolKind::Chamber(Chamber::Nursery) => "Junge wachsen doppelt so schnell heran, und Würfe werden größer.",
            ToolKind::Post => "Ein Erdhügel, von dem aus eine Wache Füchse entdeckt.",
            ToolKind::Clover => {
                "Klee wächst doppelt so schnell nach, ist nahrhafter als Gras und macht ausgelaugten Boden wieder gesund."
            }
            ToolKind::Feature(Feature::Hedge) => "Füchse kommen nur langsam hindurch, Kaninchen schlüpfen einfach durch.",
            ToolKind::Feature(Feature::BoltHole) => {
                "Ein kleines Versteck draußen: Flüchtende verschwinden darin, der Fuchs kommt nicht ran."
            }
            ToolKind::Feature(Feature::SunSpot) => "Kaninchen in der Nähe sonnen sich und werden zufriedener.",
            ToolKind::Feature(Feature::Herbs) => "Kräuter zum Fressen: Wer hier frisst, wird deutlich zufriedener.",
            ToolKind::Feature(Feature::Playground) => "Jedes Feld Spielwiese macht die ganze Familie etwas zufriedener.",
            ToolKind::Plug => {
                "Eine Stelle im Gang, die bei einem Marder sofort mit Erde verschlossen wird. \
                 Was dahinter liegt, ist sicher – aber der Weg ist dann auch für die Kaninchen zu."
            }
            ToolKind::Watch => {
                "Hier hält nachts ein Kaninchen Wache. Kommt ein Marder in den Bau, stampft es \
                 und weckt alle in den verbundenen Gängen."
            }
            ToolKind::Remove => "Aufträge, Kammern und Bauten an der Oberfläche entfernen.",
        }
    }

    /// Preis pro Feld als (Futter, Erde); `None` = kostenlos.
    pub fn price(self) -> Option<(f32, f32)> {
        match self {
            ToolKind::Post => Some((0.0, cost::POST_EARTH)),
            ToolKind::Chamber(Chamber::Sleeping) => Some((cost::NEST_FOOD, 0.0)),
            ToolKind::Clover => Some((cost::CLOVER_FOOD, 0.0)),
            ToolKind::Feature(Feature::Hedge) => Some((0.0, cost::HEDGE_EARTH)),
            ToolKind::Feature(Feature::BoltHole) => Some((0.0, cost::BOLT_HOLE_EARTH)),
            ToolKind::Feature(Feature::SunSpot) => Some((0.0, cost::SUN_SPOT_EARTH)),
            ToolKind::Feature(Feature::Herbs) => Some((cost::HERBS_FOOD, 0.0)),
            ToolKind::Feature(Feature::Playground) => Some((0.0, cost::PLAYGROUND_EARTH)),
            ToolKind::Plug => Some((0.0, cost::PLUG_EARTH)),
            ToolKind::Watch => Some((0.0, cost::WATCH_EARTH)),
            _ => None,
        }
    }

    /// Was das Werkzeug kostet, für die Anzeige.
    pub fn cost_label(self) -> Option<String> {
        let (food, earth) = self.price()?;
        let what = if food > 0.0 {
            format!("{food:.0} Futter")
        } else {
            format!("{earth:.0} Erde")
        };
        let per = match self {
            ToolKind::Chamber(Chamber::Sleeping) => " pro Nest",
            _ if self.is_single() => "",
            _ => " pro Feld",
        };
        Some(format!("{what}{per}"))
    }

    /// Welche Forschung das Werkzeug freischaltet; `None` = von Anfang an da.
    pub fn required_tech(self) -> Option<Tech> {
        match self {
            ToolKind::Post => Some(Tech::Posts),
            ToolKind::Clover => Some(Tech::Clover),
            ToolKind::Chamber(Chamber::Nursery) => Some(Tech::Nursery),
            ToolKind::Feature(Feature::Hedge) => Some(Tech::Hedges),
            ToolKind::Feature(Feature::BoltHole) => Some(Tech::BoltHoles),
            ToolKind::Feature(Feature::SunSpot) => Some(Tech::SunSpots),
            ToolKind::Feature(Feature::Herbs) => Some(Tech::Herbs),
            ToolKind::Feature(Feature::Playground) => Some(Tech::Playground),
            _ => None,
        }
    }

    /// Ab welcher Wachstumsstufe es das Werkzeug gibt; `None` = von Anfang an.
    pub fn required_stage(self) -> Option<Stage> {
        match self {
            // Marder kommen erst zur Siedlung, und mit ihnen die Mittel gegen sie.
            ToolKind::Plug | ToolKind::Watch => Some(Stage::Settlement),
            _ => None,
        }
    }

    /// Was dem Werkzeug noch fehlt, oder `None`, wenn es frei ist.
    pub fn lock(self, progress: &Progress) -> Option<Lock> {
        if let Some(tech) = self.required_tech().filter(|tech| !progress.has(*tech)) {
            return Some(Lock::Tech(tech));
        }
        self.required_stage().filter(|stage| progress.stage < *stage).map(Lock::Stage)
    }

    fn is_single(self) -> bool {
        matches!(
            self,
            ToolKind::Entrance
                | ToolKind::Post
                | ToolKind::Plug
                | ToolKind::Watch
                | ToolKind::Feature(Feature::BoltHole | Feature::SunSpot)
        )
    }

    /// Die Ansicht, in der das Werkzeug benutzt wird; `None` = beide.
    pub fn layer(self) -> Option<Layer> {
        match self {
            ToolKind::Dig | ToolKind::Chamber(_) | ToolKind::Plug | ToolKind::Watch => Some(Layer::Underground),
            ToolKind::Entrance | ToolKind::Post | ToolKind::Clover | ToolKind::Feature(_) => Some(Layer::Surface),
            ToolKind::Point | ToolKind::Remove => None,
        }
    }
}

/// Ob eine Grabtiefe schon erforscht ist.
pub fn depth_available(depth: Depth, progress: &Progress) -> bool {
    depth != Depth::Deep || progress.has(Tech::DeepTunnels)
}

/// Wählt ein Werkzeug und wechselt in die passende Ansicht, damit man sieht, was man baut.
pub fn select(kind: ToolKind, tool: &mut Tool, view: &mut ViewLayer) {
    tool.kind = kind;
    if let Some(layer) = kind.layer() {
        view.0 = layer;
    }
}

/// Meldung, wenn ein Werkzeug noch gesperrt ist.
pub fn locked_message(lock: Lock) -> String {
    match lock {
        Lock::Tech(tech) => format!("{} ist noch nicht erforscht. Öffne den Forschungsbaum mit T.", tech.label()),
        Lock::Stage(stage) => format!("Das gibt es erst ab der Stufe „{}“.", stage.label()),
    }
}

#[derive(Resource)]
pub struct Tool {
    pub kind: ToolKind,
    pub depth: Depth,
}

#[derive(Resource, Default)]
pub(crate) struct Drag {
    start: Option<TilePos>,
}

impl Drag {
    /// Der Klick galt etwas anderem, z. B. einem Kaninchen: kein Werkzeug anwenden.
    pub(crate) fn cancel(&mut self) {
        self.start = None;
    }
}

fn select_tool(
    keys: Res<ButtonInput<KeyCode>>,
    progress: Res<Progress>,
    mut tool: ResMut<Tool>,
    mut view: ResMut<ViewLayer>,
    mut messages: ResMut<Messages>,
) {
    if let Some((kind, _, _)) = TOOLS.into_iter().find(|(_, key, _)| keys.just_pressed(*key)) {
        match kind.lock(&progress) {
            Some(lock) => messages.push(locked_message(lock)),
            None => select(kind, &mut tool, &mut view),
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        // Noch nicht erforschte Tiefen überspringen.
        tool.depth = tool.depth.next();
        while !depth_available(tool.depth, &progress) {
            tool.depth = tool.depth.next();
        }
    }
}

/// Ob ein Oberflächen-Bau auf dieses Feld passt: ein Bau pro Feld, nicht auf Eingänge.
fn surface_free(field: &SurfaceTile) -> bool {
    !field.entrance && !field.entrance_planned && !field.post && !field.clover && field.feature.is_none()
}

pub(crate) fn apply_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorWorld>,
    tool: Res<Tool>,
    view: Res<ViewLayer>,
    progress: Res<Progress>,
    mut drag: ResMut<Drag>,
    mut grid: ResMut<Grid>,
    mut colony: ResMut<Colony>,
    mut messages: ResMut<Messages>,
    mut sounds: ResMut<Sounds>,
    clock: Res<DayClock>,
    toolbar: Query<&Hovered, With<Toolbar>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        // Ein Klick auf die Werkzeugleiste ist keiner in die Welt.
        let on_toolbar = toolbar.iter().any(|hovered| hovered.0);
        drag.start = if on_toolbar {
            None
        } else {
            cursor.0.and_then(TilePos::from_world)
        };
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let (Some(start), Some(cursor)) = (drag.start.take(), cursor.0) else {
        return;
    };
    if tool.kind == ToolKind::Point {
        return;
    }
    if tool.kind.layer().is_some_and(|layer| layer != view.0) {
        return;
    }
    if let Some(lock) = tool.kind.lock(&progress) {
        messages.push(locked_message(lock));
        return;
    }

    let end = TilePos::from_world_clamped(cursor);
    let frozen_work = tool.kind == ToolKind::Entrance || (tool.kind == ToolKind::Dig && tool.depth == Depth::Shallow);
    if frozen_work && clock.is_ground_frozen() {
        messages.push("Der Boden ist gefroren: Eingänge und flache Gänge werden erst im Frühling gegraben.");
    }
    // Manche Bauten immer einzeln, ein ganzer Bereich davon ergibt keinen Sinn.
    let tiles: Vec<TilePos> = if tool.kind.is_single() {
        vec![end]
    } else {
        tiles_between(start, end).collect()
    };

    let mut short = false;
    let mut too_wide = None;
    for p in tiles {
        match tool.kind {
            // Schon oben abgefangen: Zeigen baut nichts.
            ToolKind::Point => {}
            ToolKind::Dig => {
                let tile = grid.under(p);
                if tile.dug.is_some() {
                    continue;
                }
                // Schon geplante Felder wurden beim Planen geprüft.
                if tile.planned.is_none()
                    && let Some(soil) = grid.too_wide(p)
                {
                    too_wide = Some(soil);
                    continue;
                }
                grid.under_mut(p).planned = Some(tool.depth);
            }
            ToolKind::Entrance => {
                let field = grid.surface(p);
                if field.entrance || field.post || field.feature.is_some() {
                    continue;
                }
                // Unter einem Eingang entsteht ein Gang, also gilt auch hier die Breite.
                let under = grid.under(p);
                if under.dug.is_none()
                    && under.planned.is_none()
                    && let Some(soil) = grid.too_wide(p)
                {
                    too_wide = Some(soil);
                    continue;
                }
                grid.surface_mut(p).entrance_planned = true;
            }
            ToolKind::Chamber(chamber) => {
                let tile = grid.under_mut(p);
                if (tile.dug.is_none() && tile.planned.is_none()) || tile.chamber == Some(chamber) {
                    continue;
                }
                // Wird ein Nest zu etwas anderem, kommt das Gras zurück in den Vorrat.
                if tile.chamber == Some(Chamber::Sleeping) {
                    colony.food += cost::NEST_FOOD;
                }
                if chamber == Chamber::Sleeping && !colony.pay(cost::NEST_FOOD, 0.0) {
                    short = true;
                    continue;
                }
                tile.chamber = Some(chamber);
            }
            ToolKind::Plug | ToolKind::Watch => {
                let tile = grid.under(p);
                // Ein Pfropf gehört in einen Gang, eine Wache in irgendein gegrabenes Feld.
                let fits = tile.dug.is_some()
                    && !tile.plug
                    && !tile.watch
                    && (tool.kind == ToolKind::Watch || (tile.chamber.is_none() && !grid.surface(p).entrance));
                if !fits {
                    continue;
                }
                let (food, earth) = tool.kind.price().unwrap_or_default();
                if !colony.pay(food, earth) {
                    short = true;
                    continue;
                }
                let tile = grid.under_mut(p);
                if tool.kind == ToolKind::Plug {
                    tile.plug = true;
                } else {
                    tile.watch = true;
                }
            }
            ToolKind::Post | ToolKind::Clover | ToolKind::Feature(_) => {
                let field = grid.surface_mut(p);
                if !surface_free(field) {
                    continue;
                }
                let (food, earth) = tool.kind.price().unwrap_or_default();
                if !colony.pay(food, earth) {
                    short = true;
                    continue;
                }
                match tool.kind {
                    ToolKind::Post => field.post = true,
                    ToolKind::Clover => {
                        // Frisch gesät: erst einmal kurz.
                        field.clover = true;
                        field.grass = field.grass.min(0.15);
                        // Klee bindet Stickstoff: Ausgelaugter Boden wird wieder gesund.
                        field.fertility = field.fertility.max(1.0);
                    }
                    ToolKind::Feature(feature) => {
                        field.feature = Some(feature);
                        if !feature.keeps_grass() {
                            field.grass = 0.0;
                        }
                    }
                    _ => unreachable!(),
                }
            }
            ToolKind::Remove => {
                let tile = grid.under_mut(p);
                tile.planned = None;
                if std::mem::take(&mut tile.plug) {
                    colony.earth += cost::PLUG_EARTH;
                }
                if std::mem::take(&mut tile.watch) {
                    colony.earth += cost::WATCH_EARTH;
                }
                if tile.chamber.take() == Some(Chamber::Sleeping) {
                    colony.food += cost::NEST_FOOD;
                }
                let field = grid.surface_mut(p);
                field.entrance_planned = false;
                field.clover = false;
                // Erde von Hügeln und Hecken kommt zurück, Saatgut nicht.
                if std::mem::take(&mut field.post) {
                    colony.earth += cost::POST_EARTH;
                }
                if let Some(feature) = field.feature.take() {
                    let (_, earth) = ToolKind::Feature(feature).price().unwrap_or_default();
                    colony.earth += earth;
                }
            }
        }
    }
    if let Some(soil) = too_wide {
        messages.push(width_message(soil));
    }
    sounds.play(Sound::Build);
    if short {
        let price = tool.kind.cost_label().unwrap_or_default();
        messages.push(format!("Für {} reicht es nicht ganz ({price}).", tool.kind.label()));
    }
}

fn width_message(soil: Soil) -> String {
    let width = match soil.max_width() {
        1 => "1 Feld".to_string(),
        n => format!("{n} Felder"),
    };
    format!("{} hält nur Gänge, die höchstens {width} breit sind. Sonst stürzt die Decke ein.", soil.label())
}

/// Was der gerade aufgezogene Bereich kosten würde, für die Anzeige über der Werkzeugleiste.
#[derive(Resource, Default)]
pub struct SelectionCost(pub Option<String>);

/// Wie viele Felder im Bereich das Werkzeug wirklich bezahlen müsste.
fn priced_tiles(grid: &Grid, kind: ToolKind, start: TilePos, end: TilePos) -> usize {
    tiles_between(start, end)
        .filter(|p| match kind {
            ToolKind::Chamber(Chamber::Sleeping) => {
                let tile = grid.under(*p);
                (tile.dug.is_some() || tile.planned.is_some()) && tile.chamber != Some(Chamber::Sleeping)
            }
            _ => surface_free(grid.surface(*p)),
        })
        .count()
}

fn draw_preview(
    mut gizmos: Gizmos,
    buttons: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorWorld>,
    tool: Res<Tool>,
    view: Res<ViewLayer>,
    drag: Res<Drag>,
    grid: Res<Grid>,
    colony: Res<Colony>,
    mut selection: ResMut<SelectionCost>,
    toolbar: Query<&Hovered, With<Toolbar>>,
) {
    // Beim Zeigen gibt es nichts zu markieren.
    if tool.kind == ToolKind::Point {
        selection.0 = None;
        return;
    }
    let dragging = buttons.pressed(MouseButton::Left) && drag.start.is_some();
    let (Some(cursor), false) = (cursor.0, toolbar.iter().any(|hovered| hovered.0)) else {
        // Über der Werkzeugleiste keine Vorschau in der Welt darunter.
        selection.0 = None;
        return;
    };
    let info = drag.start.filter(|_| dragging && !tool.kind.is_single()).and_then(|start| {
        let (food, earth) = tool.kind.price()?;
        let end = TilePos::from_world_clamped(cursor);
        let count = priced_tiles(&grid, tool.kind, start, end);
        // Bezahlt wird in genau einem Rohstoff.
        let (price, stock, unit) = if food > 0.0 {
            (food, colony.food, "Futter")
        } else {
            (earth, colony.earth, "Erde")
        };
        let total = count as f32 * price;
        let affordable = (stock / price).floor() as usize;
        let shortfall = if affordable < count {
            format!(" Reicht nur für {affordable}.")
        } else {
            String::new()
        };
        let fields = if count == 1 { "Feld" } else { "Felder" };
        Some(format!(
            "{count} {fields} {}: {total:.0} {unit} (vorhanden: {stock:.0}).{shortfall}",
            tool.kind.label(),
        ))
    });
    // Beim Graben ohne Kosten zeigen, welcher Boden unter dem Zeiger liegt.
    let info = info.or_else(|| {
        let digging = view.0 == Layer::Underground && matches!(tool.kind, ToolKind::Dig | ToolKind::Chamber(_));
        let soil = grid.under(TilePos::from_world_clamped(cursor)).soil;
        let width = soil.max_width();
        digging.then(|| format!("Boden: {}, Gänge bis {width} breit.", soil.label()))
    });
    if selection.0 != info {
        selection.0 = info;
    }
    let wrong_view = tool.kind.layer().is_some_and(|layer| layer != view.0);
    let color = if wrong_view {
        Color::srgba(0.9, 0.3, 0.2, 0.8)
    } else {
        Color::srgba(1.0, 0.95, 0.7, 0.9)
    };

    let end = TilePos::from_world_clamped(cursor);
    let start = match drag.start {
        Some(start) if buttons.pressed(MouseButton::Left) && !tool.kind.is_single() => start,
        _ => end,
    };
    let min = start.center().min(end.center()) - TILE_SIZE / 2.0;
    let max = start.center().max(end.center()) + TILE_SIZE / 2.0;
    // Auf der Oberkante, sonst steckt der Rahmen dort, wo noch nicht gegraben ist, in der Erde.
    gizmo_rect(&mut gizmos, min, max, 0.0, color);
}
