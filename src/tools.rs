//! Bauwerkzeuge: Mit gedrückter linker Maustaste einen Bereich aufziehen.
//! Die Kaninchen arbeiten die Aufträge danach selbst ab; Bauten an der Oberfläche stehen sofort.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;

use crate::audio::{Sound, Sounds};
use crate::camera::CursorWorld;
use crate::colony::{Colony, Messages, cost};
use crate::daynight::DayClock;
use crate::ground::gizmo_rect;
use crate::map::{
    Chamber, Depth, Feature, Grid, Layer, Plot, Soil, SurfaceTile, TILE_SIZE, TilePos, ViewLayer, footprint, tiles_between,
};
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
    /// Ein angrenzendes Grundstück dazukaufen.
    Land,
    /// Trampelpfad: Darauf hoppeln Kaninchen schneller.
    Trail,
    Remove,
}

/// Gruppen der Werkzeugleiste: unten wählt man die Gruppe, oben stehen ihre Werkzeuge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolGroup {
    Point,
    Dig,
    Chambers,
    Food,
    Safety,
    Community,
    Land,
    Remove,
}

impl ToolGroup {
    pub const ALL: [ToolGroup; 8] = [
        ToolGroup::Point,
        ToolGroup::Dig,
        ToolGroup::Chambers,
        ToolGroup::Food,
        ToolGroup::Safety,
        ToolGroup::Community,
        ToolGroup::Land,
        ToolGroup::Remove,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ToolGroup::Point => "Zeigen",
            ToolGroup::Dig => "Graben",
            ToolGroup::Chambers => "Kammern",
            ToolGroup::Food => "Futter",
            ToolGroup::Safety => "Schutz",
            ToolGroup::Community => "Gemeinschaft",
            ToolGroup::Land => "Land und Wege",
            ToolGroup::Remove => "Entfernen",
        }
    }

    /// Die Werkzeuge der Gruppe, in der Reihenfolge der Leiste.
    pub fn tools(self) -> impl Iterator<Item = ToolKind> {
        TOOLS.into_iter().map(|(kind, _, _)| kind).filter(move |kind| kind.group() == self)
    }

    /// Das Werkzeug, dessen Bild die Gruppe zeigt.
    pub fn icon_tool(self) -> ToolKind {
        self.tools().next().unwrap_or(ToolKind::Point)
    }
}

/// Warum ein Werkzeug noch gesperrt ist.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lock {
    Tech(Tech),
    Stage(Stage),
}

/// Alle Werkzeuge in der Reihenfolge der Werkzeugleiste, mit Taste und Tastenname.
pub const TOOLS: [(ToolKind, KeyCode, &str); 22] = [
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
    (ToolKind::Feature(Feature::Dam), KeyCode::Digit8, "8"),
    (ToolKind::Feature(Feature::SunSpot), KeyCode::KeyO, "O"),
    (ToolKind::Clover, KeyCode::KeyK, "K"),
    (ToolKind::Feature(Feature::Herbs), KeyCode::KeyU, "U"),
    (ToolKind::Feature(Feature::Playground), KeyCode::KeyM, "M"),
    (ToolKind::Feature(Feature::Carrots), KeyCode::Digit4, "4"),
    (ToolKind::Feature(Feature::Festival), KeyCode::Digit5, "5"),
    (ToolKind::Watch, KeyCode::KeyI, "I"),
    (ToolKind::Plug, KeyCode::KeyC, "C"),
    (ToolKind::Land, KeyCode::Digit6, "6"),
    (ToolKind::Trail, KeyCode::Digit7, "7"),
    (ToolKind::Remove, KeyCode::KeyX, "X"),
];

impl ToolKind {
    pub fn group(self) -> ToolGroup {
        match self {
            ToolKind::Point => ToolGroup::Point,
            ToolKind::Dig | ToolKind::Entrance => ToolGroup::Dig,
            ToolKind::Chamber(_) => ToolGroup::Chambers,
            ToolKind::Clover | ToolKind::Feature(Feature::Herbs | Feature::Carrots) => ToolGroup::Food,
            ToolKind::Post
            | ToolKind::Watch
            | ToolKind::Plug
            | ToolKind::Feature(Feature::Hedge | Feature::BoltHole | Feature::Dam) => ToolGroup::Safety,
            ToolKind::Feature(Feature::SunSpot | Feature::Playground | Feature::Festival) => ToolGroup::Community,
            ToolKind::Land | ToolKind::Trail => ToolGroup::Land,
            ToolKind::Remove => ToolGroup::Remove,
        }
    }

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
            ToolKind::Land => "Land kaufen",
            ToolKind::Trail => "Trampelpfad",
            ToolKind::Remove => "Entfernen",
        }
    }

    /// Ein Satz dazu, was das Werkzeug tut, für den Tooltip.
    pub fn description(self) -> &'static str {
        match self {
            ToolKind::Point => "Kaninchen anklicken und ansehen, ohne etwas zu bauen. Esc wechselt jederzeit hierher.",
            ToolKind::Dig => "Gänge und Kammern ausheben lassen. Bringt Erde. Mit größerer Tiefe über bestehende Gänge gezogen, werden sie tiefer gelegt: je Stufe noch einmal Erde.",
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
            ToolKind::Feature(Feature::Dam) => {
                "Ein Erdwall gegen Hochwasser: Das Wasser kommt nicht hindurch, Eingänge dahinter bleiben trocken. \
                 Wie beim Pfad zieht man ihn als Linie."
            }
            ToolKind::Feature(Feature::Herbs) => "Kräuter zum Fressen: Wer hier frisst, wird deutlich zufriedener.",
            ToolKind::Feature(Feature::Playground) => "Jedes Feld Spielwiese macht die ganze Familie etwas zufriedener.",
            ToolKind::Feature(Feature::Carrots) => "Möhren wachsen langsam, machen aber dreimal so satt wie Gras.",
            ToolKind::Feature(Feature::Festival) => {
                "Ein großer Platz (3 × 3 Felder), auf dem die Familie feiert: Wer kein Nest hat, ist weniger \n                 unglücklich und zieht erst nach vier Nächten fort."
            }
            ToolKind::Plug => {
                "Eine Stelle im Gang, die bei einem Marder sofort mit Erde verschlossen wird. \
                 Was dahinter liegt, ist sicher – aber der Weg ist dann auch für die Kaninchen zu."
            }
            ToolKind::Watch => {
                "Hier hält nachts ein Kaninchen Wache. Kommt ein Marder in den Bau, stampft es \
                 und weckt alle in den verbundenen Gängen."
            }
            ToolKind::Land => {
                "Ein Grundstück neben eurem Land für Erde dazukaufen. Wie viel Land ihr haben dürft, \
                 hängt von der Größe der Familie ab."
            }
            ToolKind::Trail => {
                "Vom Start zum Ziel ziehen: Der Pfad geht erst waagerecht, dann senkrecht, mit einer \
                 Kurve an der Ecke. Darauf hoppeln Kaninchen ein Drittel schneller, auch auf der Flucht \
                 vor dem Fuchs. Das Gras wächst weiter."
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
            ToolKind::Feature(Feature::Carrots) => Some((cost::CARROTS_FOOD, 0.0)),
            ToolKind::Feature(Feature::Festival) => Some((0.0, cost::FESTIVAL_EARTH)),
            ToolKind::Feature(Feature::Dam) => Some((0.0, cost::DAM_EARTH)),
            ToolKind::Trail => Some((0.0, cost::TRAIL_EARTH)),
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
            ToolKind::Feature(Feature::Carrots) => Some(Tech::Carrots),
            ToolKind::Feature(Feature::Festival) => Some(Tech::Festival),
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
        // Kennt die Familie Nachtwache und Erdpfropf schon aus früheren Kapiteln, gibt es sie gleich.
        if matches!(self, ToolKind::Plug | ToolKind::Watch) && progress.night_tools {
            return None;
        }
        self.required_stage().filter(|stage| progress.stage < *stage).map(Lock::Stage)
    }

    fn is_single(self) -> bool {
        matches!(
            self,
            ToolKind::Entrance
                | ToolKind::Post
                | ToolKind::Land
                | ToolKind::Plug
                | ToolKind::Watch
                | ToolKind::Feature(Feature::BoltHole | Feature::SunSpot | Feature::Festival)
        )
    }

    /// Die Ansicht, in der das Werkzeug benutzt wird; `None` = beide.
    pub fn layer(self) -> Option<Layer> {
        match self {
            ToolKind::Dig | ToolKind::Chamber(_) | ToolKind::Plug | ToolKind::Watch => Some(Layer::Underground),
            ToolKind::Entrance | ToolKind::Post | ToolKind::Clover | ToolKind::Trail | ToolKind::Feature(_) => {
                Some(Layer::Surface)
            }
            ToolKind::Point | ToolKind::Land | ToolKind::Remove => None,
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
    !field.entrance && !field.entrance_planned && !field.post && !field.clover && field.feature.is_none() && !field.water
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
    if tool.kind == ToolKind::Land {
        buy_plot(Plot::of(TilePos::from_world_clamped(cursor)), &mut grid, &mut colony, &progress, &mut messages, &mut sounds);
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
    let tiles: Vec<TilePos> = drag_tiles(tool.kind, start, end);

    if let ToolKind::Feature(feature) = tool.kind
        && feature.size() > 1
    {
        build_big(feature, end, &mut grid, &mut colony, &mut messages);
        return;
    }

    let mut short = false;
    let mut too_wide = None;
    let mut unexplored = false;
    for p in tiles {
        // Auf fremdem Land wird nichts gebaut.
        if !grid.owns(p) && tool.kind != ToolKind::Remove {
            unexplored = true;
            continue;
        }
        match tool.kind {
            // Schon oben abgefangen: Zeigen baut nichts, Land wird einzeln gekauft.
            ToolKind::Point | ToolKind::Land => {}
            ToolKind::Dig => {
                let tile = grid.under(p);
                // Ein bestehender Gang lässt sich tiefer legen, das bringt noch einmal Erde.
                // Unter Eingängen bleibt er flach, dort geht es nach draußen.
                if let Some(now) = tile.dug {
                    if tool.depth.level() > now.level() && !grid.surface(p).entrance {
                        grid.under_mut(p).planned = Some(tool.depth);
                    }
                    continue;
                }
                // Schon geplante Felder wurden beim Planen geprüft.
                if tile.planned.is_none()
                    && let Some(soil) = grid.too_wide_with(p, width_bonus(&progress))
                {
                    too_wide = Some(soil);
                    continue;
                }
                grid.under_mut(p).planned = Some(tool.depth);
            }
            ToolKind::Entrance => {
                let field = grid.surface(p);
                if field.entrance || field.post || field.feature.is_some() || field.water {
                    continue;
                }
                // Unter einem Eingang entsteht ein Gang, also gilt auch hier die Breite.
                let under = grid.under(p);
                if under.dug.is_none()
                    && under.planned.is_none()
                    && let Some(soil) = grid.too_wide_with(p, width_bonus(&progress))
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
            ToolKind::Trail => {
                let field = grid.surface(p);
                if field.trail || field.entrance || field.water {
                    continue;
                }
                if !colony.pay(0.0, cost::TRAIL_EARTH) {
                    short = true;
                    continue;
                }
                grid.surface_mut(p).trail = true;
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
                field.trail = false;
                // Erde von Hügeln und Hecken kommt zurück, Saatgut nicht.
                if std::mem::take(&mut field.post) {
                    colony.earth += cost::POST_EARTH;
                }
                remove_feature(&mut grid, &mut colony, p);
            }
        }
    }
    if let Some(soil) = too_wide {
        messages.push(width_message(soil, width_bonus(&progress)));
    }
    if unexplored {
        messages.push("Dieses Land gehört euch noch nicht. Kauft es mit „Land kaufen“ (6) dazu.");
    }
    sounds.play(Sound::Build);
    if short {
        let price = tool.kind.cost_label().unwrap_or_default();
        messages.push(format!("Für {} reicht es nicht ganz ({price}).", tool.kind.label()));
    }
}

/// Was das nächste Grundstück kostet: jedes zugekaufte etwas mehr.
fn plot_price(grid: &Grid) -> f32 {
    cost::PLOT_EARTH * (grid.owned.len() + 1 - Plot::starting().len()) as f32
}

/// Warum dieses Grundstück gerade nicht zu kaufen ist, oder `None`, wenn es geht.
fn plot_blocked(plot: Plot, grid: &Grid, colony: &Colony, progress: &Progress) -> Option<String> {
    if grid.owned.contains(&plot) {
        return Some("Dieses Land gehört euch schon.".into());
    }
    if !grid.can_buy(plot) {
        return Some("Kaufen lässt sich nur Land direkt neben eurem.".into());
    }
    if grid.owned.len() >= progress.plots_allowed() {
        return Some(match progress.stage.next() {
            Some(next) => format!("Mehr Land gibt es erst ab der Stufe „{}“.", next.label()),
            None => "Ihr habt schon alles Land.".into(),
        });
    }
    let price = plot_price(grid);
    (colony.earth + colony.food < price).then(|| {
        format!("Dafür reicht es nicht: {price:.0} nötig, {:.0} Erde und {:.0} Futter da.", colony.earth, colony.food)
    })
}

/// Wie ein Grundstück bezahlt wird: erst mit Erde, was fehlt, mit Futter.
fn plot_payment(grid: &Grid, colony: &Colony) -> (f32, f32) {
    let price = plot_price(grid);
    let earth = colony.earth.min(price).max(0.0);
    (earth, price - earth)
}

fn buy_plot(
    plot: Plot,
    grid: &mut Grid,
    colony: &mut Colony,
    progress: &Progress,
    messages: &mut Messages,
    sounds: &mut Sounds,
) {
    if let Some(reason) = plot_blocked(plot, grid, colony, progress) {
        messages.push(reason);
        return;
    }
    let (earth, food) = plot_payment(grid, colony);
    colony.earth -= earth;
    colony.food -= food;
    grid.owned.push(plot);
    sounds.play(Sound::Research);
    messages.push(if food > 0.0 {
        format!("Neues Land gekauft, für {earth:.0} Erde und {food:.0} Futter! Hier kann die Familie jetzt grasen und bauen.")
    } else {
        "Neues Land gekauft! Hier kann die Familie jetzt grasen und bauen.".to_string()
    });
}

/// Mit Stützwurzeln trägt jeder Boden einen Gang mehr in der Breite.
fn width_bonus(progress: &Progress) -> i32 {
    i32::from(progress.has(Tech::SupportRoots))
}

fn width_message(soil: Soil, bonus: i32) -> String {
    if soil == Soil::Rock {
        return "Hier ist Fels, da kann man nicht graben.".into();
    }
    let width = match soil.max_width() + bonus {
        1 => "1 Feld".to_string(),
        n => format!("{n} Felder"),
    };
    format!("{} hält nur Gänge, die höchstens {width} breit sind. Sonst stürzt die Decke ein.", soil.label())
}

/// Was der gerade aufgezogene Bereich kosten würde, für die Anzeige über der Werkzeugleiste.
#[derive(Resource, Default)]
pub struct SelectionCost(pub Option<String>);

/// Wie viele Felder im Bereich das Werkzeug wirklich bezahlen müsste.
/// Ein Pfad von A nach B mit einer Ecke: erst waagerecht vom Start aus, dann senkrecht zum Ziel.
/// An der Ecke entsteht so von selbst eine Kurve.
fn trail_route(start: TilePos, end: TilePos) -> Vec<TilePos> {
    let step = |from: i32, to: i32| if to >= from { 1 } else { -1 };
    let mut route = Vec::new();
    let mut x = start.x;
    loop {
        route.push(TilePos::new(x, start.y));
        if x == end.x {
            break;
        }
        x += step(start.x, end.x);
    }
    let mut y = start.y;
    while y != end.y {
        y += step(start.y, end.y);
        route.push(TilePos::new(end.x, y));
    }
    route
}

/// Welche Felder ein Ziehen von `start` bis `end` mit diesem Werkzeug betrifft.
fn drag_tiles(kind: ToolKind, start: TilePos, end: TilePos) -> Vec<TilePos> {
    match kind {
        _ if kind.is_single() => vec![end],
        ToolKind::Trail | ToolKind::Feature(Feature::Dam) => trail_route(start, end),
        _ => tiles_between(start, end).collect(),
    }
}

/// Reißt den Bau auf `p` ab und gibt seine Erde zurück; ein großer Bau verschwindet ganz,
/// egal welches seiner Felder man trifft.
fn remove_feature(grid: &mut Grid, colony: &mut Colony, p: TilePos) {
    let field = grid.surface_mut(p);
    let Some(feature) = field.feature.take() else {
        return;
    };
    let (_, earth) = ToolKind::Feature(feature).price().unwrap_or_default();
    colony.earth += earth;
    if let Some(center) = field.part_of.take() {
        for q in footprint(center, feature.size()) {
            let part = grid.surface_mut(q);
            if part.part_of == Some(center) {
                part.feature = None;
                part.part_of = None;
            }
        }
    }
}

/// Warum ein großer Bau hier nicht passt, oder `None`, wenn alle seine Felder frei sind.
fn big_blocked(feature: Feature, center: TilePos, grid: &Grid) -> Option<String> {
    let size = feature.size();
    let fits = footprint(center, size).all(|q| q.in_bounds() && grid.owns(q) && surface_free(grid.surface(q)));
    (!fits).then(|| format!("{} braucht {size} × {size} freie Felder auf eigenem Land.", feature.label()))
}

/// Setzt einen großen Bau mit dem Mittelfeld `center` und bezahlt ihn einmal.
fn build_big(feature: Feature, center: TilePos, grid: &mut Grid, colony: &mut Colony, messages: &mut Messages) {
    if let Some(reason) = big_blocked(feature, center, grid) {
        messages.push(reason);
        return;
    }
    let (food, earth) = ToolKind::Feature(feature).price().unwrap_or_default();
    if !colony.pay(food, earth) {
        messages.push(format!("Für {} fehlt Erde: {earth:.0} nötig.", feature.label()));
        return;
    }
    for q in footprint(center, feature.size()) {
        let field = grid.surface_mut(q);
        field.feature = Some(feature);
        field.part_of = Some(center);
        if !feature.keeps_grass() {
            field.grass = 0.0;
        }
    }
}

fn priced_tiles(grid: &Grid, kind: ToolKind, start: TilePos, end: TilePos) -> usize {
    drag_tiles(kind, start, end)
        .into_iter()
        .filter(|p| match kind {
            ToolKind::Chamber(Chamber::Sleeping) => {
                let tile = grid.under(*p);
                (tile.dug.is_some() || tile.planned.is_some()) && tile.chamber != Some(Chamber::Sleeping)
            }
            ToolKind::Trail => {
                let field = grid.surface(*p);
                !field.trail && !field.entrance && !field.water
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
    progress: Res<Progress>,
    mut selection: ResMut<SelectionCost>,
    toolbar: Query<&Hovered, With<Toolbar>>,
) {
    // Beim Zeigen gibt es nichts zu markieren.
    if tool.kind == ToolKind::Point {
        selection.0 = None;
        return;
    }
    // Beim Landkauf: kaufbare Grundstücke gelb umrandet, über dem Zeiger Preis oder Hinderungsgrund.
    if tool.kind == ToolKind::Land {
        let Some(cursor) = cursor.0 else {
            selection.0 = None;
            return;
        };
        let hovered = Plot::of(TilePos::from_world_clamped(cursor));
        for plot in Plot::all() {
            if grid.owned.contains(&plot) {
                continue;
            }
            let buyable = plot_blocked(plot, &grid, &colony, &progress).is_none();
            let color = match (buyable, plot == hovered) {
                (true, true) => Color::srgb(1.0, 0.95, 0.5),
                (true, false) => Color::srgba(0.98, 0.82, 0.3, 0.8),
                (false, _) => Color::srgba(0.6, 0.6, 0.6, 0.5),
            };
            let half = TILE_SIZE / 2.0;
            gizmo_rect(&mut gizmos, plot.first_tile().center() - half + 4.0, plot.last_tile().center() + half - 4.0, 0.0, color);
        }
        let info = match plot_blocked(hovered, &grid, &colony, &progress) {
            None => match plot_payment(&grid, &colony) {
                (_, food) if food > 0.0 => format!(
                    "Land kaufen: {:.0} Erde. Es fehlen {food:.0}, die zahlt ihr mit Futter (vorhanden: {:.0} Erde, {:.0} Futter).",
                    plot_price(&grid),
                    colony.earth,
                    colony.food,
                ),
                _ => format!("Land kaufen: {:.0} Erde (vorhanden: {:.0}).", plot_price(&grid), colony.earth),
            },
            Some(reason) => reason,
        };
        if selection.0.as_ref() != Some(&info) {
            selection.0 = Some(info);
        }
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
        let width = soil.max_width() + width_bonus(&progress);
        digging.then(|| match soil {
            Soil::Rock => "Boden: Fels, hier kann man nicht graben.".to_string(),
            _ => format!("Boden: {}, Gänge bis {width} breit.", soil.label()),
        })
    });
    // Ein großer Bau zeigt, was er kostet oder warum er hier nicht hinpasst.
    let big = match tool.kind {
        ToolKind::Feature(feature) if feature.size() > 1 => Some(feature),
        _ => None,
    };
    let info = info.or_else(|| {
        let feature = big?;
        let center = TilePos::from_world_clamped(cursor);
        let (_, earth) = tool.kind.price()?;
        Some(big_blocked(feature, center, &grid).unwrap_or_else(|| {
            format!("{}: {earth:.0} Erde (vorhanden: {:.0}).", feature.label(), colony.earth)
        }))
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
    // Ein großer Bau zeigt seine ganze Fläche, rot, wenn sie nicht frei ist.
    if let Some(feature) = big {
        let blocked = big_blocked(feature, end, &grid).is_some();
        let color = if blocked { Color::srgba(0.9, 0.3, 0.2, 0.8) } else { color };
        let reach = (feature.size() / 2) as f32 * TILE_SIZE + TILE_SIZE / 2.0;
        gizmo_rect(&mut gizmos, end.center() - reach, end.center() + reach, 0.0, color);
        return;
    }
    // Ein Pfad zeigt seinen Weg Feld für Feld, alles andere den aufgezogenen Bereich.
    if matches!(tool.kind, ToolKind::Trail | ToolKind::Feature(Feature::Dam)) {
        for p in trail_route(start, end) {
            let half = TILE_SIZE / 2.0 - 6.0;
            gizmo_rect(&mut gizmos, p.center() - half, p.center() + half, 0.0, color);
        }
        return;
    }
    let min = start.center().min(end.center()) - TILE_SIZE / 2.0;
    let max = start.center().max(end.center()) + TILE_SIZE / 2.0;
    // Auf der Oberkante, sonst steckt der Rahmen dort, wo noch nicht gegraben ist, in der Erde.
    gizmo_rect(&mut gizmos, min, max, 0.0, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trail_goes_across_then_down_with_one_corner() {
        let route = trail_route(TilePos::new(26, 18), TilePos::new(20, 15));
        assert_eq!(route.first(), Some(&TilePos::new(26, 18)));
        assert_eq!(route.last(), Some(&TilePos::new(20, 15)));
        // 7 Felder waagerecht, dann 3 senkrecht, ohne Lücke.
        assert_eq!(route.len(), 10);
        assert!(route.windows(2).all(|w| (w[0].x - w[1].x).abs() + (w[0].y - w[1].y).abs() == 1));
        assert!(route.contains(&TilePos::new(20, 18)), "die Ecke liegt über dem Ziel");
    }

    #[test]
    fn land_is_paid_with_earth_first_and_the_rest_with_food() {
        let grid = Grid::starting_meadow();
        let mut progress = Progress::default();
        progress.stage = crate::progress::Stage::Warren;
        let plot = Plot::of(TilePos::new(5, 15));
        let price = plot_price(&grid);

        let colony = Colony { earth: price - 4.0, food: 10.0, ..default() };
        assert_eq!(plot_blocked(plot, &grid, &colony, &progress), None, "Futter springt ein");
        assert_eq!(plot_payment(&grid, &colony), (price - 4.0, 4.0));

        let rich = Colony { earth: price + 5.0, food: 10.0, ..default() };
        assert_eq!(plot_payment(&grid, &rich), (price, 0.0), "genug Erde: kein Futter");

        let poor = Colony { earth: price - 4.0, food: 3.0, ..default() };
        assert!(plot_blocked(plot, &grid, &poor, &progress).is_some(), "zusammen zu wenig");
    }

    #[test]
    fn a_festival_is_built_and_removed_as_a_whole() {
        let mut grid = Grid::starting_meadow();
        let mut colony = Colony { earth: 20.0, ..default() };
        let mut messages = Messages::default();
        let center = TilePos::new(16, 17);
        build_big(Feature::Festival, center, &mut grid, &mut colony, &mut messages);
        assert_eq!(colony.earth, 20.0 - cost::FESTIVAL_EARTH, "einmal bezahlt, nicht pro Feld");
        assert!(footprint(center, 3).all(|q| grid.surface(q).part_of == Some(center)));

        // Ein zweiter, der sich mit dem ersten überschneidet, passt nicht hin.
        colony.earth = 100.0;
        build_big(Feature::Festival, TilePos::new(18, 17), &mut grid, &mut colony, &mut messages);
        assert_eq!(colony.earth, 100.0);

        // Abreißen an einer Ecke entfernt alle neun Felder.
        remove_feature(&mut grid, &mut colony, TilePos::new(15, 16));
        assert!(footprint(center, 3).all(|q| grid.surface(q).feature.is_none()));
        assert_eq!(colony.earth, 100.0 + cost::FESTIVAL_EARTH);
    }
}
