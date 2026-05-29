use super::*;
use super::{DirtyBelt, dir::Curve, inventory::Stack, should_ramp};

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (determine_belt_shape, ApplyDeferred, determine_belt_shape).chain(),
                move_items_on_belts,
                transfer_items,
                set_item_transforms,
                fill_sources,
                fill_miners,
                push_to_belt,
                (recalculate_filters, pull_from_belt, tick_collectors).chain(),
                process_furnace,
                process_assembler,
                super::player::process_hand_crafter,
                consume_sink_buffer,
                side_loading,
                grow_corn,
            ),
        );
        app.add_systems(PostUpdate, despawn_old_entities);
    }
}

fn despawn_old_entities(mut cmd: Commands, q: Query<Entity, With<Delete>>) {
    for entity in q {
        cmd.entity(entity).despawn();
    }
}

fn determine_belt_shape(
    mut belts: Query<
        (Entity, &WorldCoords, &HDir, Option<&mut BeltShape>),
        (With<Belt>, Or<(Added<Belt>, With<DirtyBelt>)>),
    >,
    belts_check: Query<(), With<Belt>>,
    affecters: Query<&OutputsToBelt>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    for (entity, coords, &dir, current_shape) in belts.iter_mut() {
        if let Ok(a) = affecters.get(entity)
            && let Some(&a) = coord_map.0.get(&a.at)
            && let Ok(()) = belts_check.get(a)
        {
            continue;
        }
        let a_feeds_b = |a: WorldCoords, b: WorldCoords| {
            coord_map
                .0
                .get(&a)
                .and_then(|a| affecters.get(*a).ok())
                .is_some_and(|a| a.at == b)
        };
        let fed_from_side = |loc: WorldCoords, fed_from: HDir| {
            let middle_fed = loc.step(fed_from);
            a_feeds_b(middle_fed, loc)
                || a_feeds_b(middle_fed.step(Dir::Up), loc)
                || a_feeds_b(middle_fed.step(Dir::Down), loc)
        };

        let fed_from_left = fed_from_side(*coords, dir.left());
        let fed_from_right = fed_from_side(*coords, dir.right());
        let fed_from_behind = fed_from_side(*coords, dir.opposite());
        debug!(
            "Placing with: {:?}",
            (fed_from_left, fed_from_behind, fed_from_right)
        );
        let desired = match (fed_from_left, fed_from_behind, fed_from_right) {
            (true, false, false) => {
                let curve = Curve::from_input_output(dir.right(), dir).unwrap();
                assert_eq!(curve.output(), dir);
                BeltShape::Curve(curve)
            }
            (false, false, true) => {
                let curve = Curve::from_input_output(dir.left(), dir).unwrap();
                assert_eq!(curve.output(), dir);
                BeltShape::Curve(curve)
            }
            (false, false, false) => BeltShape::Straight(dir),
            (_, true, _) => BeltShape::Straight(dir),
            (true, _, true) => BeltShape::Straight(dir),
        };
        let desired = if matches!(desired, BeltShape::Straight(_)) {
            should_ramp(dir, *coords, &coord_map).unwrap_or(desired)
        } else {
            desired
        };
        let output = coords.step(desired);
        match current_shape {
            Some(mut shape) => {
                if *shape != desired {
                    shape.set_if_neq(desired);
                    if let Some(&e) = coord_map.0.get(&output) {
                        cmd.entity(e).insert(DirtyBelt);
                    }
                }
            }
            None => {
                if let Some(&e) = coord_map.0.get(&output) {
                    cmd.entity(e).insert(DirtyBelt);
                }
                cmd.entity(entity).insert(desired);
            }
        }
        cmd.entity(entity).insert(OutputsToBelt { at: output });
        cmd.entity(entity).remove::<DirtyBelt>();
    }
}

fn move_items_on_belts(mut belts: Query<(&mut ItemLanes, &BeltShape)>) {
    for mut belt in belts.iter_mut() {
        for side in SIDES {
            let Some(lead_item) = belt.0.0[side].get_mut(0) else {
                continue;
            };
            lead_item.0 = 0.max(lead_item.0 - BASE_BELT_SPEED);
            for i in 1..belt.0.0[side].len() {
                let first = belt.0.0[side][i - 1];
                let second = &mut belt.0.0[side][i];

                second.0 = (first.0 + ITEM_SPACING).max(second.0 - BASE_BELT_SPEED);
            }
        }
    }
}

fn transfer_items(
    mut invs: Query<(Entity, &mut ItemLanes, &WorldCoords, &BeltShape)>,
    coord_map: Res<CoordsMap>,
) {
    struct Transfer {
        source: Entity,
        dest: Entity,
        side: Side,
    }
    let mut transfers = Vec::new();
    for source in invs.iter() {
        let next = source.2.step(source.3.belt_output());
        let Some(&dest_entity) = coord_map.0.get(&next) else {
            continue;
        };
        let Ok(dest) = invs.get(dest_entity) else {
            continue;
        };
        for side in SIDES {
            let Some(i) = source.1.0[side].get(0) else {
                continue;
            };
            if i.0 <= 0
                && dest.1.0[side].last().map(|a| a.0).unwrap_or(0) + ITEM_SPACING
                    < dest.3.num_pos(side)
                && source.3.output() == dest.3.input()
            {
                transfers.push(Transfer {
                    source: source.0,
                    dest: dest_entity,
                    side,
                });
            }
        }
    }
    for transfer in transfers {
        let mut source = invs.get_mut(transfer.source).unwrap();
        let slot = source.1.0[transfer.side].remove(0);
        drop(source);

        let mut dest = invs.get_mut(transfer.dest).unwrap();
        let lane = &mut dest.1.0[transfer.side];
        lane.push((dest.3.num_pos(transfer.side), slot.1));
    }
}

fn side_loading(
    mut invs: Query<(Entity, &mut ItemLanes, &WorldCoords, &BeltShape)>,
    coord_map: Res<CoordsMap>,
) {
    struct Transfer {
        source: Entity,
        dest: Entity,
        source_side: Side,
        dest_side: Side,
        position: ItemPos,
    }
    let mut transfers = Vec::new();
    for source in invs.iter() {
        let next = source.2.step(source.3.belt_output());
        let Some(&dest_entity) = coord_map.0.get(&next) else {
            continue;
        };
        let Ok(dest) = invs.get(dest_entity) else {
            continue;
        };
        if matches!(
            dest.3,
            BeltShape::Straight(_) | BeltShape::RampUp(_) | BeltShape::RampDown(_)
        ) && (source.3.output() == dest.3.input().left()
            || source.3.output() == dest.3.input().right())
        {
            let dest_side = if source.3.output() == dest.3.input().right() {
                Side::Left
            } else {
                Side::Right
            };
            for side in SIDES {
                let Some(item) = source.1.0[side].get(0) else {
                    continue;
                };
                if item.0 <= 0
                    && dest.1.0[dest_side].last().map(|a| a.0).unwrap_or(0) + ITEM_SPACING
                        < dest.3.num_pos(dest_side)
                {
                    const OFFSET: i32 = (POSITIONS_PER_BELT as f32 * LANE_OFFSET).round() as i32;
                    let position = if side == dest_side {
                        POSITIONS_PER_BELT / 2 - OFFSET
                    } else {
                        POSITIONS_PER_BELT / 2 + OFFSET
                    };
                    transfers.push(Transfer {
                        source: source.0,
                        dest: dest_entity,
                        source_side: side,
                        dest_side,
                        position,
                    });
                }
            }
        }
    }
    for transfer in transfers {
        let mut source = invs.get_mut(transfer.source).unwrap();
        let slot = source.1.0[transfer.source_side].remove(0);
        drop(source);

        let mut dest = invs.get_mut(transfer.dest).unwrap();
        let lane = &mut dest.1.0[transfer.dest_side];
        lane.push((transfer.position, slot.1));
    }
}

fn set_item_transforms(
    belts: Query<(&ItemLanes, &BeltShape, &WorldCoords, &HDir)>,
    mut items: Query<&mut Transform, With<OnBelt>>,
) {
    for belt in belts {
        for side in SIDES {
            for slot in belt.0.0[side].iter() {
                let Ok(mut item) = items.get_mut(slot.1) else {
                    continue;
                };
                *item = item_position(*belt.1, *belt.2, side, slot.0);
            }
        }
    }
}

fn fill_sources(mut sources: Query<(&Source, &mut OutputBuffer)>) {
    for (source, mut buffer) in &mut sources {
        if let Some(item) = source.configured_item {
            buffer.insert(&[item.into()]);
        }
    }
}

fn grow_corn(mut corns: Query<&mut Corn>, corn_ticks: Res<CornGrowthTicks>) {
    let ticks_per_stage = corn_ticks.0 / 3;
    for mut corn in &mut corns {
        let age = match corn.bypass_change_detection() {
            Corn::Growing { age } => *age,
            Corn::FullyGrown => continue,
        };

        let new_age = age + 1;
        if new_age >= corn_ticks.0 {
            *corn = Corn::FullyGrown;
        } else if new_age % ticks_per_stage == 0 {
            match &mut *corn {
                Corn::Growing { age } => *age = new_age,
                Corn::FullyGrown => {}
            }
        } else {
            match corn.bypass_change_detection() {
                Corn::Growing { age } => *age = new_age,
                Corn::FullyGrown => {}
            }
        }
    }
}

fn fill_miners(
    mut miners: Query<(&WorldCoords, &mut Miner, &mut OutputBuffer)>,
    world_blocks: Query<&Structure>,
    coord_map: Res<CoordsMap>,
    miner_ticks: Res<MinerTicksPerExtract>,
) {
    for (miner_coords, mut miner, mut buffer) in &mut miners {
        miner.ticks += 1;
        if miner.ticks < miner_ticks.0 {
            continue;
        }
        miner.ticks = 0;

        let Some(&item) = coord_map.0.get(&miner_coords.step(miner.dir)) else {
            continue;
        };
        let Ok(block) = world_blocks.get(item) else {
            continue;
        };
        let Some(item) = block.mine() else {
            continue;
        };
        let stack: Stack = item.into();
        if !buffer.would_overflow(&[stack]) {
            buffer.insert(&[stack]);
        }
    }
}

fn push_to_belt(
    mut pushers: Query<(&mut OutputBuffer, &WorldCoords, &OutputsToBelt)>,
    mut belts: Query<(Entity, &mut ItemLanes), With<Belt>>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    for (mut buffer, _coords, output_dir) in &mut pushers {
        let target = output_dir.at;
        let Some(&belt_entity) = coord_map.0.get(&target) else {
            continue;
        };
        let Ok((belt_entity, mut lanes)) = belts.get_mut(belt_entity) else {
            continue;
        };
        if lanes.0.left.len() >= ITEMS_PER_BELT as usize {
            continue;
        }
        let Some(item) = buffer.remove_any() else {
            continue;
        };
        let entity = cmd.spawn(OnBelt).id();
        lanes.0.left.push((POSITIONS_PER_BELT, entity));
        cmd.trigger(PlaceItem {
            entity,
            item: item.item,
        });
    }
}

fn pull_from_belt(
    mut sinks: Query<(&mut InputBuffer, &WorldCoords, Option<&Filter>)>,
    mut belts: Query<(&mut ItemLanes, &HDir)>,
    items: Query<&Item, With<OnBelt>>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
) {
    for (mut buffer, sink_coords, filter) in &mut sinks {
        for d in [HDir::North, HDir::South, HDir::East, HDir::West] {
            let neighbor = sink_coords.step(d.opposite());
            let Some(&belt_entity) = coord_map.0.get(&neighbor) else {
                continue;
            };
            let Ok((mut lanes, belt_dir)) = belts.get_mut(belt_entity) else {
                continue;
            };
            if *belt_dir != d {
                continue;
            }
            for side in SIDES {
                let Some(lead_item) = lanes.0[side].get(0) else {
                    continue;
                };
                if lead_item.0 != 0 {
                    continue;
                }
                let item_entity = lead_item.1;
                let Ok(&item) = items.get(item_entity) else {
                    continue;
                };
                if !filter.map_or(true, |f| f.accepts(item)) {
                    continue;
                }
                lanes.0[side].remove(0);
                cmd.entity(item_entity).despawn();
                buffer.insert(&[item.into()]);
                break;
            }
        }
    }
}

fn tick_collectors(
    mut collectors: Query<(&mut Collector, &WorldCoords, &HDir)>,
    mut belts: Query<(&mut ItemLanes, &BeltShape), With<Belt>>,
    items: Query<&Item, With<OnBelt>>,
    mut output_buffers: Query<&mut OutputBuffer>,
    mut input_buffers: Query<(&mut InputBuffer, Option<&Filter>)>,
    mut visual_transforms: Query<&mut Transform>,
    coord_map: Res<CoordsMap>,
    mut cmd: Commands,
    collector_ticks: Res<CollectorMoveTicks>,
) {
    for (mut collector, &coords, &dir) in &mut collectors {
        let forward = coords.step(dir);
        let backward = coords.step(dir.opposite());
        let forward_ent = coord_map.0.get(&forward).copied();
        let backward_ent = coord_map.0.get(&backward).copied();

        let pickup_pos = Vec3::from(backward) + Vec3::new(0.0, BELT_HEIGHT + 0.5, 0.0);
        let dropoff_pos = Vec3::from(forward) + Vec3::new(0.0, BELT_HEIGHT + 0.5, 0.0);

        let state = collector.state;
        match state {
            CollectorState::ReadyToPickUp => {
                let Some(bwd) = backward_ent else { continue };

                let forward_filter: Option<Filter> = forward_ent
                    .and_then(|fwd_ent| input_buffers.get(fwd_ent).ok())
                    .and_then(|(_, filter)| filter.cloned());

                let maybe_item: Option<Item> = if let Ok((mut lanes, _)) = belts.get_mut(bwd) {
                    let mut taken = None;
                    for side in SIDES {
                        if let Some(&(pos, item_ent)) = lanes.0[side].get(0) {
                            if pos == 0 {
                                if let Ok(&item) = items.get(item_ent) {
                                    if forward_filter.as_ref().map_or(true, |f| f.accepts(item)) {
                                        lanes.0[side].remove(0);
                                        cmd.entity(item_ent).despawn();
                                        taken = Some(item);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    taken
                } else if let Ok(mut out_buf) = output_buffers.get_mut(bwd) {
                    let peeked = out_buf.slots.get(0).map(|s| s.item);
                    match peeked {
                        Some(item) if forward_filter.as_ref().map_or(true, |f| f.accepts(item)) => {
                            out_buf.remove_any().map(|s| s.item)
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                if let Some(item) = maybe_item {
                    let visual = cmd.spawn(Transform::from_translation(pickup_pos)).id();
                    collector.state = CollectorState::MovingItem {
                        item,
                        visual,
                        start: pickup_pos,
                        end: dropoff_pos,
                        ticks: 1,
                        needs_place_item: true,
                    };
                }
            }
            CollectorState::MovingItem {
                item,
                visual,
                start,
                end,
                ticks,
                needs_place_item,
            } => {
                if needs_place_item {
                    cmd.trigger(PlaceItem {
                        entity: visual,
                        item,
                    });
                }
                let t = (ticks as f32 / collector_ticks.0 as f32).min(1.0);
                let pos = start.lerp(end, t);
                if let Ok(mut transform) = visual_transforms.get_mut(visual) {
                    transform.translation = pos;
                }
                if ticks >= collector_ticks.0 {
                    collector.state = CollectorState::ReadyToDropOff { item, visual };
                } else {
                    collector.state = CollectorState::MovingItem {
                        item,
                        visual,
                        start,
                        end,
                        ticks: ticks + 1,
                        needs_place_item: false,
                    };
                }
            }
            CollectorState::ReadyToDropOff { item, visual } => {
                let Some(fwd) = forward_ent else { continue };

                let deposited = if let Ok((mut in_buf, filter)) = input_buffers.get_mut(fwd) {
                    if filter.map_or(true, |f| f.accepts(item)) {
                        in_buf.insert(&[item.into()]);
                        cmd.entity(visual).despawn();
                        true
                    } else {
                        false
                    }
                } else if let Ok((mut lanes, shape)) = belts.get_mut(fwd) {
                    let mut placed = false;
                    for side in SIDES {
                        let last_pos = lanes.0[side].last().map(|&(p, _)| p).unwrap_or(0);
                        let n_pos = shape.num_pos(side);
                        if (lanes.0[side].len() as i32) < ITEMS_PER_BELT
                            && last_pos + ITEM_SPACING <= n_pos
                        {
                            let new_ent = cmd.spawn(OnBelt).id();
                            lanes.0[side].push((n_pos, new_ent));
                            cmd.trigger(PlaceItem {
                                entity: new_ent,
                                item,
                            });
                            cmd.entity(visual).despawn();
                            placed = true;
                            break;
                        }
                    }
                    placed
                } else {
                    false
                };

                if deposited {
                    collector.state = CollectorState::MovingToStart {
                        ticks: collector_ticks.0,
                    };
                }
            }
            CollectorState::MovingToStart { ticks } => {
                if ticks == 0 {
                    collector.state = CollectorState::ReadyToPickUp;
                } else {
                    collector.state = CollectorState::MovingToStart { ticks: ticks - 1 };
                }
            }
        }
    }
}

fn recalculate_filters(
    mut furnaces: Query<(&Furnace, &InputBuffer, &mut Filter), Without<Assembler>>,
    mut assemblers: Query<(&Assembler, &InputBuffer, &mut Filter), Without<Furnace>>,
    recipes: Res<Recipes>,
) {
    let furnace_recipes: Vec<machine::FurnaceRecipe> = recipes
        .0
        .iter()
        .filter_map(|r| {
            if let Recipe::FurnaceRecipe(fr) = r {
                Some(*fr)
            } else {
                None
            }
        })
        .collect();

    for (furnace, input, mut filter) in &mut furnaces {
        *filter = furnace.allowed_items(input, &furnace_recipes);
    }

    for (assembler, input, mut filter) in &mut assemblers {
        *filter = assembler.allowed_items(input);
    }
}

fn process_furnace(
    mut furnaces: Query<(&mut Furnace, &mut InputBuffer, &mut OutputBuffer)>,
    recipes: Res<Recipes>,
) {
    let furnace_recipes: Vec<FurnaceRecipe> = recipes
        .0
        .iter()
        .filter_map(|r| match r {
            Recipe::FurnaceRecipe(fr) => Some(*fr),
            _ => None,
        })
        .collect();

    for (mut furnace, mut input, mut output) in &mut furnaces {
        furnace.tick(&mut input, &mut output, &furnace_recipes);
    }
}

fn process_assembler(mut assemblers: Query<(&mut Assembler, &mut InputBuffer, &mut OutputBuffer)>) {
    for (mut assembler, mut input, mut output) in &mut assemblers {
        assembler.tick(&mut input, &mut output);
    }
}

fn consume_sink_buffer(mut sinks: Query<&mut InputBuffer, With<Sink>>) {
    for mut buffer in &mut sinks {
        for slot in &mut buffer.slots {
            slot.count = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::inventory::{Inventory, Stack};
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};

    fn test_app() -> App {
        let mut app = super::super::test_app();
        app.add_plugins(SimPlugin);
        app
    }

    #[test]
    fn single_belt_is_straight_north() {
        single_belt_is_straight(HDir::North);
    }

    #[test]
    fn single_belt_is_straight_south() {
        single_belt_is_straight(HDir::South);
    }

    #[test]
    fn single_belt_is_straight_east() {
        single_belt_is_straight(HDir::East);
    }

    #[test]
    fn single_belt_is_straight_west() {
        single_belt_is_straight(HDir::West);
    }

    fn single_belt_is_straight(dir: HDir) {
        let mut app = test_app();

        let belt = app.add_belt(WorldCoords::ORIGIN, dir);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(dir));
    }

    #[test]
    fn flat_belt_curves() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        app.add_belt(o.step(HDir::South), HDir::North);
        app.update();

        let belt = app.add_belt(o, HDir::West);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Curve(Curve::NorthToWest));
    }

    #[test]
    fn incline_belt() {
        let mut app = test_app();
        let belt = app.add_belt(WorldCoords::ORIGIN, HDir::North);
        app.update();

        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::RampUp(HDir::North));
    }

    #[test]
    fn incline_belt_with_belt_in_front() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        let belt = app.add_belt(o, HDir::North);
        app.add_belt(o.step(HDir::North), HDir::North);
        app.update();

        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::RampUp(HDir::North));
    }

    #[test]
    fn incline_belt_on_placement() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        app.add_belt(o.step(HDir::North).step(Dir::Up), HDir::North);
        app.update();

        let belt = app.add_belt(o, HDir::North);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::RampUp(HDir::North));
    }

    #[test]
    fn not_incline_belt_on_placement_in_front() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        app.add_belt(o.step(HDir::North).step(Dir::Up), HDir::North);
        app.add_belt(o.step(HDir::North), HDir::North);
        app.update();

        let belt = app.add_belt(o, HDir::North);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn not_incline_belt_on_placement_above() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        app.add_belt(o.step(HDir::North).step(Dir::Up), HDir::North);
        app.add_belt(o.step(Dir::Up), HDir::North);
        app.update();

        let belt = app.add_belt(o, HDir::North);
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn incline_with_above_filled_becomes_ramp_down() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        let belt = app.add_belt(o, HDir::North);
        app.add_belt(o.step(Dir::Up), HDir::North);
        app.update();

        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::RampDown(HDir::North));
    }

    #[test]
    fn incline_ramp_down_becomes_straight() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        let belt = app.add_belt(o, HDir::North);
        app.add_belt(o.step(Dir::Up), HDir::North);
        app.update();

        // First incline: Straight -> RampDown (above filled)
        app.world_mut().trigger(Incline { entity: belt });
        app.update();
        // Second incline: RampDown -> Straight
        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn incline_ramp_up_with_below_filled_becomes_straight() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;
        let belt = app.add_belt(o.step(Dir::Up), HDir::North);
        app.add_belt(o, HDir::North);
        app.update();

        // First incline: Straight -> RampUp (nothing above)
        app.world_mut().trigger(Incline { entity: belt });
        app.update();
        let b = app.find_belt(belt).unwrap();
        assert_eq!(b.0, BeltShape::RampUp(HDir::North));

        // Second incline: RampUp -> Straight (below filled, can't ramp down)
        app.world_mut().trigger(Incline { entity: belt });
        app.update();

        let belt = app.find_belt(belt).unwrap();
        assert_eq!(belt.0, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn miner_does_not_extract_without_adjacent_ore() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;

        // Place iron ore deposit two steps away — not adjacent to the miner.
        app.add_world_block(
            o.step(HDir::South).step(HDir::South),
            Structure::IronOreDeposit,
        );

        let miner = app.world_mut().spawn_empty().id();
        let flb = o;
        app.world_mut()
            .resource_scope(|world, mut coord_map: Mut<CoordsMap>| {
                let mut cmd = world.commands();
                let mut ec = cmd.entity(miner);
                Structure::Miner.attach_bundle(&mut ec, &mut *coord_map, flb, Some(HDir::South));
            });
        app.world_mut().flush();

        let belt = app.add_belt(o.step(HDir::North), HDir::North);

        let miner_ticks = app.world().resource::<MinerTicksPerExtract>().0;
        for _ in 0..=miner_ticks {
            app.update();
        }

        assert_eq!(app.item_count_on_belt(belt), 0);
    }

    #[test]
    fn miner_extracts_ore_onto_belt() {
        let mut app = test_app();
        let o = WorldCoords::ORIGIN;

        // Place iron ore deposit adjacent to the south of the miner position.
        app.add_world_block(o.step(HDir::South), Structure::IronOreDeposit);

        // Place miner at origin facing the ore to the south.
        let miner = app.world_mut().spawn_empty().id();
        let flb = o;
        app.world_mut()
            .resource_scope(|world, mut coord_map: Mut<CoordsMap>| {
                let mut cmd = world.commands();
                let mut ec = cmd.entity(miner);
                Structure::Miner.attach_bundle(&mut ec, &mut *coord_map, flb, Some(HDir::South));
            });
        app.world_mut().flush();

        // Place belt to the north — the miner's OutputDir(None) will find it.
        let belt = app.add_belt(o.step(HDir::North), HDir::North);

        // Tick until the miner has had enough time to extract and push.
        let miner_ticks = app.world().resource::<MinerTicksPerExtract>().0;
        for _ in 0..=miner_ticks {
            app.update();
        }

        assert!(app.item_count_on_belt(belt) > 0);
    }

    #[test]
    fn miner_outputs_correct_ore_for_deposit() {
        for (deposit, expected_ore) in [
            (Structure::IronOreDeposit, Item::IronOre),
            (Structure::CopperOreDeposit, Item::CopperOre),
        ] {
            let mut app = test_app();
            let o = WorldCoords::ORIGIN;

            app.add_world_block(o.step(HDir::South), deposit);

            let miner = app.world_mut().spawn_empty().id();
            let flb = o;
            app.world_mut()
                .resource_scope(|world, mut coord_map: Mut<CoordsMap>| {
                    let mut cmd = world.commands();
                    let mut ec = cmd.entity(miner);
                    Structure::Miner.attach_bundle(
                        &mut ec,
                        &mut *coord_map,
                        flb,
                        Some(HDir::South),
                    );
                });
            app.world_mut().flush();

            let belt = app.add_belt(o.step(HDir::North), HDir::North);

            let miner_ticks = app.world().resource::<MinerTicksPerExtract>().0;
            for _ in 0..=miner_ticks {
                app.update();
            }

            let world = app.world_mut();
            let lanes = world.query::<&ItemLanes>().get(world, belt).unwrap();
            let item_entities: Vec<Entity> = lanes
                .0
                .left
                .iter()
                .chain(lanes.0.right.iter())
                .map(|(_, e)| *e)
                .collect();
            assert!(
                !item_entities.is_empty(),
                "expected ore on belt for {deposit:?}"
            );
            for entity in item_entities {
                let item = *world.query::<&Item>().get(world, entity).unwrap();
                assert_eq!(item, expected_ore, "wrong ore for {deposit:?}");
            }
        }
    }

    #[test]
    fn belt_ramping_down_curves_belt_in_front_on_incline() {
        let mut app = test_app();
        let ramp = app.add_belt(WorldCoords::ORIGIN.step(Dir::Up), HDir::North);
        let curve = app.add_belt(WorldCoords::ORIGIN.step(Dir::North), HDir::East);
        app.update();

        app.world_mut().trigger(Incline { entity: ramp });
        app.update();

        let (c, _) = app.find_belt(curve).unwrap();
        assert_eq!(c, BeltShape::Curve(Curve::NorthToEast));
    }

    #[test]
    fn belt_ramping_down_curves_belt_in_front_on_place() {
        let mut app = test_app();
        let ramp = app.add_belt(WorldCoords::ORIGIN.step(Dir::South), HDir::North);
        app.update();
        app.world_mut().trigger(Incline { entity: ramp });
        app.update();

        let curve = app.add_belt(WorldCoords::ORIGIN.step(Dir::Up), HDir::East);
        app.update();

        let (c, _) = app.find_belt(curve).unwrap();
        assert_eq!(c, BeltShape::Curve(Curve::NorthToEast));
    }

    #[test]
    fn belt_curves_belt_in_front() {
        let mut app = test_app();
        let curve = app.add_belt(WorldCoords::ORIGIN, HDir::East);
        app.update();

        app.add_belt(WorldCoords::ORIGIN.step(Dir::South), HDir::North);
        app.update();

        let (c, _) = app.find_belt(curve).unwrap();
        assert_eq!(c, BeltShape::Curve(Curve::NorthToEast));
    }

    #[test]
    fn belt_two_side_load_is_straight() {
        let mut app = test_app();
        app.add_belt(WorldCoords::ORIGIN.step(HDir::West), HDir::East);
        app.add_belt(WorldCoords::ORIGIN.step(HDir::East), HDir::West);
        app.update();

        let belt = app.add_belt(WorldCoords::ORIGIN, HDir::North);
        app.update();

        let (c, _) = app.find_belt(belt).unwrap();
        assert_eq!(c, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn belt_side_load_and_back_load_is_straight() {
        let mut app = test_app();
        app.add_belt(WorldCoords::ORIGIN.step(HDir::South), HDir::North);
        app.add_belt(WorldCoords::ORIGIN.step(HDir::East), HDir::West);
        app.update();

        let belt = app.add_belt(WorldCoords::ORIGIN, HDir::North);
        app.update();

        let (c, _) = app.find_belt(belt).unwrap();
        assert_eq!(c, BeltShape::Straight(HDir::North));
    }

    #[test]
    fn belt_remp_doesnt_update_when_pointing_at_belt() {
        let mut app = test_app();
        app.add_belt(
            WorldCoords::ORIGIN.step(Dir::Up).step(HDir::North),
            HDir::North,
        );
        app.add_belt(WorldCoords::ORIGIN.step(HDir::North), HDir::North);
        app.update();
        let ramp = app.add_belt(WorldCoords::ORIGIN, HDir::North);
        app.update();

        app.world_mut().trigger(Incline { entity: ramp });
        app.update();
        app.add_belt(WorldCoords::ORIGIN.step(HDir::South), HDir::North);
        app.update();

        let (c, _) = app.find_belt(ramp).unwrap();
        assert_eq!(c, BeltShape::RampUp(HDir::North));
    }

    #[test]
    fn load_machine_input_moves_ore_to_furnace_input_buffer() {
        let mut app = test_app();

        let player = app.spawn_player();
        let furnace = app.add_world_block(WorldCoords::ORIGIN, Structure::Furnace);
        app.update();

        // Give the player 2 iron ore and find which slot they land in.
        let ore_slot = {
            let mut inv = app.world_mut().get_mut::<Inventory>(player).unwrap();
            inv.insert(Stack::new(Item::IronOre, 2)).unwrap();

            (0..64)
                .find(|&s| {
                    inv.get(s)
                        .map(|st| st.item == Item::IronOre)
                        .unwrap_or(false)
                })
                .expect("player should have iron ore")
        };

        app.world_mut().trigger(LoadMachineInput {
            player,
            player_inventory_slot: ore_slot,
            machine: furnace,
            machine_input_slot: None,
        });
        app.update();

        let input_buf = app.world().get::<InputBuffer>(furnace).unwrap();
        assert!(
            input_buf
                .slots
                .iter()
                .any(|s| s.item == Item::IronOre && s.count >= 1),
            "expected at least 1 iron ore in furnace input buffer, got: {:?}",
            input_buf.slots,
        );
    }

    #[test]
    fn collector_deposits_item_into_furnace() {
        let mut app = test_app();

        // Layout (top = North = -z):
        //   furnace at (0, 0, -2)  →  occupies z=-2 and z=-1
        //   collector at (0, 0, 0) facing North
        //     forward  = (0,0,-1) → inside furnace footprint
        //     backward = (0,0, 1) → belt
        //   belt at (0, 0, 1) facing North (exits toward collector)

        let furnace = {
            let e = app.world_mut().spawn_empty().id();
            let flb: WorldCoords = (0i32, 0i32, -2i32).into();
            app.world_mut()
                .resource_scope(|world, mut coord_map: Mut<CoordsMap>| {
                    let mut cmd = world.commands();
                    let mut ec = cmd.entity(e);
                    Structure::Furnace.attach_bundle(
                        &mut ec,
                        &mut *coord_map,
                        flb,
                        Some(HDir::North),
                    );
                });
            app.world_mut().flush();
            e
        };
        let _collector = {
            let e = app.world_mut().spawn_empty().id();
            let flb: WorldCoords = (0i32, 0i32, 0i32).into();
            app.world_mut()
                .resource_scope(|world, mut coord_map: Mut<CoordsMap>| {
                    let mut cmd = world.commands();
                    let mut ec = cmd.entity(e);
                    Structure::Collector.attach_bundle(
                        &mut ec,
                        &mut *coord_map,
                        flb,
                        Some(HDir::North),
                    );
                });
            app.world_mut().flush();
            e
        };
        let belt = app.add_belt((0i32, 0i32, 1i32), HDir::North);
        app.update(); // flush deferred component inserts

        // Manually add an IronOre item at position 0 on the belt (head = exit end)
        let item_entity = app
            .world_mut()
            .spawn((OnBelt, Item::IronOre, Transform::default()))
            .id();
        app.world_mut().get_mut::<ItemLanes>(belt).unwrap().0[Side::Left].push((0, item_entity));

        // Check CoordsMap has the expected entries
        {
            let coord_map = app.world().resource::<CoordsMap>();
            let furnace_cell: WorldCoords = (0i32, 0i32, -1i32).into();
            let belt_cell: WorldCoords = (0i32, 0i32, 1i32).into();
            assert!(
                coord_map.0.contains_key(&furnace_cell),
                "CoordsMap should have furnace at (0,0,-1)"
            );
            assert!(
                coord_map.0.contains_key(&belt_cell),
                "CoordsMap should have belt at (0,0,1)"
            );
        }

        // Tick 1: collector should pick up the item
        app.update();
        {
            let world = app.world_mut();
            let collector_state = world.query::<&Collector>().single(world).unwrap().state;
            assert!(
                matches!(collector_state, CollectorState::MovingItem { .. }),
                "after tick 1, collector should be MovingItem, got: {:?}",
                collector_state
            );
        }

        let collector_move_ticks = app.world().resource::<CollectorMoveTicks>().0;
        for _ in 0..collector_move_ticks {
            app.update();
        }

        {
            let world = app.world_mut();
            let collector_state = world.query::<&Collector>().single(world).unwrap().state;
            assert!(
                matches!(collector_state, CollectorState::ReadyToDropOff { .. })
                    || matches!(collector_state, CollectorState::MovingToStart { .. }),
                "after move ticks, collector should be ReadyToDropOff or MovingToStart, got: {:?}",
                collector_state
            );
        }

        // A few more ticks to ensure deposit happens
        for _ in 0..5 {
            app.update();
        }

        // The furnace consumes the IronOre from InputBuffer immediately when it starts
        // processing. So check that the furnace is now processing (or has produced output).
        let furnace_component = app.world().get::<Furnace>(furnace).unwrap();
        let input_buf = app.world().get::<InputBuffer>(furnace).unwrap();
        let output_buf = app.world().get::<OutputBuffer>(furnace).unwrap();
        assert!(
            matches!(furnace_component.status, MachineStatus::Processing { .. })
                || output_buf.slots.iter().any(|s| s.item == Item::IronIngot)
                || input_buf.slots.iter().any(|s| s.item == Item::IronOre),
            "expected furnace to be processing iron ore (collector delivered it), \
             got status: {:?}, input: {:?}, output: {:?}",
            furnace_component.status,
            input_buf.slots,
            output_buf.slots,
        );
    }
}
