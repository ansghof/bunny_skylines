//! Maulwurfshügel: Ab und zu wirft ein Maulwurf auf eigenem Land einen Hügel lockerer Erde auf.
//! Kommt ein Kaninchen darüber, ebnet es ihn ein, und die Erde geht in den Vorrat. So versiegt
//! die Erde nie ganz, auch wenn alles Land ausgegraben ist.

use bevy::prelude::*;

use crate::colony::{Colony, Messages, Rng};
use crate::daynight::DayClock;
use crate::map::{Grid, Layer, TILE_SIZE, TilePos};
use crate::rabbit::{OnLayer, Rabbit};

/// So viel Erde bringt ein eingeebneter Hügel.
pub const MOLEHILL_EARTH: f32 = 3.0;
/// Mehr Hügel liegen nie gleichzeitig herum.
const MAX_MOLEHILLS: usize = 4;
/// Spielsekunden zwischen zwei neuen Hügeln, von bis.
const MOLEHILL_SECONDS: (f32, f32) = (40.0, 80.0);

pub struct MolehillPlugin;

impl Plugin for MolehillPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MoleClock>().add_systems(Update, (raise_molehills, level_molehills));
    }
}

#[derive(Resource)]
struct MoleClock {
    next: f32,
    told: bool,
}

impl Default for MoleClock {
    fn default() -> Self {
        Self { next: MOLEHILL_SECONDS.0, told: false }
    }
}

/// Kann hier ein Hügel entstehen? Nur auf freier, eigener Wiese.
fn room_for_molehill(grid: &Grid, p: TilePos) -> bool {
    let tile = grid.surface(p);
    grid.owns(p)
        && !(tile.molehill
            || tile.entrance
            || tile.entrance_planned
            || tile.post
            || tile.water
            || tile.stones
            || tile.tree
            || tile.trail
            || tile.flooded
            || tile.feature.is_some()
            || tile.part_of.is_some())
}

fn raise_molehills(
    time: Res<Time>,
    clock: Res<DayClock>,
    mut grid: ResMut<Grid>,
    mut moles: ResMut<MoleClock>,
    mut rng: ResMut<Rng>,
    mut messages: ResMut<Messages>,
) {
    // Im gefrorenen Boden gräbt auch kein Maulwurf.
    if clock.is_ground_frozen() {
        return;
    }
    moles.next -= time.delta_secs();
    if moles.next > 0.0 {
        return;
    }
    moles.next = rng.range(MOLEHILL_SECONDS.0, MOLEHILL_SECONDS.1);
    if Grid::positions().filter(|p| grid.surface(*p).molehill).count() >= MAX_MOLEHILLS {
        return;
    }
    let free: Vec<TilePos> = Grid::positions().filter(|p| room_for_molehill(&grid, *p)).collect();
    if free.is_empty() {
        return;
    }
    // Maulwürfe mögen kurzes Gras: Von ein paar zufälligen Feldern nimmt er das am meisten
    // abgegraste. So liegen die Hügel meist dort, wo die Kaninchen ohnehin unterwegs sind.
    let p = (0..4)
        .map(|_| free[rng.below(free.len())])
        .min_by(|a, b| grid.surface(*a).grass.total_cmp(&grid.surface(*b).grass))
        .unwrap();
    grid.surface_mut(p).molehill = true;
    if !moles.told {
        moles.told = true;
        messages.push("Ein Maulwurf hat einen Hügel aufgeworfen. Läuft ein Kaninchen darüber, ebnet es ihn ein: Das gibt Erde.");
    }
}

/// So nah muss ein Kaninchen an die Mitte eines Hügels kommen, um ihn einzuebnen.
const LEVEL_REACH: f32 = TILE_SIZE * 0.9;

/// Ein Kaninchen, das über oder dicht an einem Hügel vorbei hoppelt, ebnet ihn ein.
fn level_molehills(
    mut grid: ResMut<Grid>,
    mut colony: ResMut<Colony>,
    mut messages: ResMut<Messages>,
    rabbits: Query<(&Rabbit, &Transform, &OnLayer)>,
) {
    for (rabbit, transform, layer) in &rabbits {
        if layer.0 != Layer::Surface {
            continue;
        }
        let at = transform.translation.truncate();
        let here = TilePos::from_world_clamped(at);
        let near = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| TilePos::new(here.x + dx, here.y + dy)));
        for p in near {
            if p.in_bounds() && grid.surface(p).molehill && p.center().distance(at) < LEVEL_REACH {
                grid.surface_mut(p).molehill = false;
                colony.earth += MOLEHILL_EARTH;
                messages.push(format!("{} hat einen Maulwurfshügel eingeebnet: {MOLEHILL_EARTH:.0} Erde.", rabbit.name));
            }
        }
    }
}
