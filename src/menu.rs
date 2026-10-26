//! Titelbild mit Hauptmenü beim Start und ein Pausenmenü im Spiel (F10 oder Knopf in der Leiste).
//! Hinter den Knöpfen liegt eine abgedunkelte Fläche, damit die Schrift auf dem Bild lesbar bleibt.

use bevy::app::AppExit;
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::ending::{EndScreen, restart, spawn_button, spawn_overlay};
use crate::save::{load_game, save_exists, save_game};
use crate::scenario::{show_intro, start_playing};
use crate::toolbar::Toolbar;

/// Bild für den Titel, relativ zum Ordner `assets`.
const TITLE_IMAGE: &str = "ui/titelbild.png";
/// So lange steht das Titelbild allein da, bevor das Menü erscheint.
const SPLASH_SECONDS: f32 = 1.5;
/// Füllt links und rechts neben dem quadratischen Bild, passend zum Grün der Wiese.
const SIDE_COLOR: Color = Color::srgb(0.16, 0.24, 0.12);
const SHADE: Color = Color::srgba(0.06, 0.05, 0.03, 0.72);
const BUTTON: Color = Color::srgb(0.45, 0.33, 0.22);
const BUTTON_HOVER: Color = Color::srgb(0.58, 0.43, 0.28);
const TEXT: Color = Color::srgb(0.98, 0.95, 0.88);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_title)
            .init_resource::<OpenMenu>()
            .add_systems(Update, (reveal_menu, hover_buttons, pause_menu_key));
    }
}

#[derive(Component)]
pub struct TitleScreen;

/// Der Menüknopf in der Werkzeugleiste bittet hierüber um das Pausenmenü.
#[derive(Resource, Default)]
pub struct OpenMenu(pub bool);

/// Das Menü auf dem Titelbild; erscheint nach dem Splash oder beim ersten Klick.
#[derive(Component)]
struct TitleMenu;

#[derive(Component)]
struct MenuButton;

fn font(size: f32) -> TextFont {
    TextFont {
        font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
        ..TextFont::from_font_size(size)
    }
}

/// Liegt das Titelbild schon im Spielordner? Sonst gibt es einen schlichten Ersatz.
fn title_image_exists() -> bool {
    let mut roots = vec![std::path::PathBuf::from("assets")];
    if let Some(dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        roots.push(std::path::PathBuf::from(dir).join("assets"));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.join("assets"))) {
        roots.push(dir);
    }
    roots.iter().any(|root| root.join(TITLE_IMAGE).exists())
}

fn spawn_title(mut commands: Commands, asset_server: Res<AssetServer>, mut time: ResMut<Time<Virtual>>) {
    time.pause();
    let screen = commands
        .spawn((
            TitleScreen,
            // Wie die anderen Fenster: hält das Tempo an und schluckt Klicks.
            EndScreen,
            Toolbar,
            Hovered::default(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(SIDE_COLOR),
            GlobalZIndex(20),
        ))
        .id();

    if title_image_exists() {
        commands.spawn((
            ImageNode::new(asset_server.load(TITLE_IMAGE)),
            Node {
                height: Val::Percent(100.0),
                aspect_ratio: Some(1.0),
                ..default()
            },
            ChildOf(screen),
        ));
    } else {
        commands.spawn((
            Text::new("Bunny Skylines\nBau ’nen Bau!"),
            TextLayout::justify(Justify::Center),
            TextColor(TEXT),
            font(56.0),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(14.0),
                ..default()
            },
            ChildOf(screen),
        ));
    }

    // Das Menü: unten in der Mitte, auf abgedunkeltem Grund.
    let menu = commands
        .spawn((
            TitleMenu,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                bottom: Val::Percent(18.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(36.0), Val::Px(22.0)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(SHADE),
            ChildOf(screen),
        ))
        .id();
    if save_exists() {
        menu_button(&mut commands, menu, "Weiterspielen").observe(on_primary(continue_game));
    }
    menu_button(&mut commands, menu, "Neues Spiel").observe(on_primary(new_game));
    menu_button(&mut commands, menu, "Beenden").observe(on_primary(quit));
}

fn menu_button<'a>(commands: &'a mut Commands, parent: Entity, label: &str) -> EntityCommands<'a> {
    let button = commands
        .spawn((
            MenuButton,
            Hovered::default(),
            Node {
                width: Val::Px(260.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(22.0), Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(BUTTON),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((Text::new(label), TextColor(TEXT), font(22.0), ChildOf(button)));
    commands.entity(button)
}

/// Ein Beobachter, der bei Linksklick das System `action` ausführt.
fn on_primary<M: 'static>(
    action: impl IntoSystem<(), (), M> + Copy + Send + Sync + 'static,
) -> impl FnMut(On<PointerClick>, Commands) {
    move |click: On<PointerClick>, mut commands: Commands| {
        if click.button == PointerButton::Primary {
            commands.run_system_cached(action);
        }
    }
}

fn reveal_menu(
    time: Res<Time<Real>>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut menus: Query<&mut Node, With<TitleMenu>>,
) {
    let early = buttons.get_just_pressed().len() > 0 || keys.get_just_pressed().len() > 0;
    if time.elapsed_secs() < SPLASH_SECONDS && !early {
        return;
    }
    for mut node in &mut menus {
        if node.display == Display::None {
            node.display = Display::Flex;
        }
    }
}

fn hover_buttons(mut buttons: Query<(&Hovered, &mut BackgroundColor), (With<MenuButton>, Changed<Hovered>)>) {
    for (hovered, mut background) in &mut buttons {
        background.0 = if hovered.0 { BUTTON_HOVER } else { BUTTON };
    }
}

fn close_title(world: &mut World) {
    let mut titles = world.query_filtered::<Entity, With<TitleScreen>>();
    let titles: Vec<Entity> = titles.iter(world).collect();
    for title in titles {
        world.despawn(title);
    }
}

/// Neues Spiel vom Titelbild: Die frische Wiese steht schon bereit, es folgt die Einleitung.
pub fn new_game(world: &mut World) {
    close_title(world);
    world.run_system_cached(show_intro).ok();
}

fn continue_game(world: &mut World) {
    close_title(world);
    world.run_system_cached(load_game).ok();
}

fn quit(mut exit: MessageWriter<AppExit>) {
    exit.write(AppExit::Success);
}

/// F10 öffnet im Spiel das Pausenmenü, solange kein anderes Fenster offen ist.
fn pause_menu_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut requested: ResMut<OpenMenu>,
    screens: Query<(), With<EndScreen>>,
    mut commands: Commands,
) {
    let wanted = keys.just_pressed(KeyCode::F10) || std::mem::take(&mut requested.0);
    if wanted && screens.is_empty() {
        commands.run_system_cached(open_pause_menu);
    }
}

pub fn open_pause_menu(mut commands: Commands, mut time: ResMut<Time<Virtual>>, screens: Query<(), With<EndScreen>>) {
    if !screens.is_empty() {
        return;
    }
    time.pause();
    let panel = spawn_overlay(&mut commands, "Pause", "Die Kaninchen warten, bis es weitergeht.");
    spawn_button(&mut commands, panel, "Weiterspielen").observe(on_primary(start_playing));
    spawn_button(&mut commands, panel, "Speichern").observe(on_primary(save_and_resume));
    if save_exists() {
        spawn_button(&mut commands, panel, "Spielstand laden").observe(on_primary(load_game));
    }
    spawn_button(&mut commands, panel, "Neu beginnen").observe(on_primary(restart));
    spawn_button(&mut commands, panel, "Beenden").observe(on_primary(quit));
}

fn save_and_resume(world: &mut World) {
    save_game(world);
    world.run_system_cached(start_playing).ok();
}
