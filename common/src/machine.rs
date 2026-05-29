use bevy::platform::collections::HashSet;

use crate::inventory::Stack;
use crate::*;
use std::ops::Deref;
use std::ops::DerefMut;

#[derive(Component, Default, Debug, PartialEq)]
pub struct InputBuffer {
    pub buffer: Buffer,
}

impl Deref for InputBuffer {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for InputBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct Buffer {
    pub slots: Vec<Stack>,
}

impl Buffer {
    pub fn insert(&mut self, items: &[Stack]) {
        for stack in items {
            if let Some((index, _)) = self
                .slots
                .iter()
                .enumerate()
                .find(|(_, s)| s.item == stack.item)
            {
                self.slots[index].count += stack.count;
            } else {
                self.slots.push(*stack);
            }
        }
    }

    pub fn contains(&self, items: &[Stack]) -> bool {
        items.iter().all(|stack| {
            self.slots
                .iter()
                .find(|s| s.item == stack.item)
                .is_some_and(|s| s.count >= stack.count)
        })
    }

    pub fn remove(&mut self, items: &[Stack]) {
        assert!(self.contains(items));
        items.iter().for_each(|stack| {
            if let Some(s) = self.slots.iter_mut().find(|s| s.item == stack.item) {
                s.count -= stack.count;
            }
        });
        self.clean();
    }

    fn clean(&mut self) {
        self.slots.extract_if(.., |s| s.count == 0).for_each(|_| {})
    }

    pub fn remove_any(&mut self) -> Option<Stack> {
        let item = self.slots.get(0)?.item;
        self.slots[0].count -= 1;
        self.clean();
        Some(item.into())
    }

    pub fn view(&self) -> Vec<Stack> {
        self.slots.clone()
    }

    fn count_of(&self, item: Item) -> u16 {
        self.slots
            .iter()
            .find(|s| s.item == item)
            .map(|s| s.count)
            .unwrap_or(0)
    }

    /// Returns true if inserting `stacks` would push any item type over its
    /// stack size limit, given what's already in this buffer.
    ///
    /// When a recipe output itself exceeds one stack (edge case), this buffer
    /// must be completely empty of that item before another cycle can start.
    pub fn would_overflow(&self, stacks: &[Stack]) -> bool {
        stacks.iter().any(|stack| {
            let current = self.count_of(stack.item);
            let stack_size = stack.item.stack_size();
            if stack.count >= stack_size {
                current > 0
            } else {
                current + stack.count > stack_size
            }
        })
    }
}

#[derive(Component, Default, Debug, PartialEq)]
pub struct OutputBuffer {
    pub buffer: Buffer,
}

impl Deref for OutputBuffer {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for OutputBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FurnaceRecipe {
    pub input: Stack,
    pub output: Stack,
    pub ticks: u32,
}

#[derive(Debug, Clone)]
pub struct AssemblerRecipe {
    pub input: Vec<Stack>,
    pub output: Vec<Stack>,
    pub ticks: u32,
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct Furnace {
    #[reflect(ignore)]
    pub status: MachineStatus<FurnaceRecipe>,
}

impl Furnace {
    pub fn tick(
        &mut self,
        input: &mut InputBuffer,
        output: &mut OutputBuffer,
        recipes: &[FurnaceRecipe],
    ) {
        match &mut self.status {
            MachineStatus::Idle => {
                if let Some(recipe) = recipes.iter().find(|r| input.contains(&[r.input])) {
                    if !output.would_overflow(&[recipe.output]) {
                        input.remove(&[recipe.input]);
                        self.status = MachineStatus::Processing {
                            recipe: *recipe,
                            elapsed_ticks: 1,
                        };
                    }
                }
            }
            MachineStatus::Processing { elapsed_ticks, .. } => {
                *elapsed_ticks += 1;
            }
        }

        match &mut self.status {
            MachineStatus::Idle => {}
            MachineStatus::Processing {
                recipe,
                elapsed_ticks,
            } => {
                if *elapsed_ticks >= recipe.ticks {
                    output.insert(&[recipe.output]);
                    self.status = MachineStatus::Idle;
                }
            }
        }
    }

    pub fn allowed_items(&self, input: &InputBuffer, recipes: &[FurnaceRecipe]) -> Filter {
        // Find recipe inputs currently in the buffer (auto-selected)
        let selected: Vec<(Item, u16, u16)> = recipes
            .iter()
            .filter_map(|r| {
                let buffer_count = input
                    .slots
                    .iter()
                    .find(|s| s.item == r.input.item)
                    .map(|s| s.count)
                    .unwrap_or(0);
                if buffer_count > 0 {
                    Some((r.input.item, buffer_count, r.input.count))
                } else {
                    None
                }
            })
            .collect();

        if selected.is_empty() {
            // Nothing selected yet — accept any recipe input
            Filter::from_iter(recipes.iter().map(|r| r.input.item))
        } else {
            // Selected on specific item(s); accept only those that aren't full (< 2x needed)
            Filter::from_iter(selected.into_iter().filter_map(|(item, count, needed)| {
                if count < needed * 2 { Some(item) } else { None }
            }))
        }
    }
}

impl WorldDrop for Furnace {
    fn drop_items(&self) -> Vec<Stack> {
        let mut drops = vec![Stack::from(Item::Furnace)];
        if let MachineStatus::Processing { recipe, .. } = &self.status {
            drops.push(recipe.input);
        }
        drops
    }
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct Assembler {
    #[reflect(ignore)]
    pub status: MachineStatus<AssemblerRecipe>,
    #[reflect(ignore)]
    pub configured_recipe: Option<AssemblerRecipe>,
}

impl Assembler {
    pub fn tick(&mut self, input: &mut InputBuffer, output: &mut OutputBuffer) {
        let recipe = self.configured_recipe.clone();
        match &mut self.status {
            MachineStatus::Idle => {
                if let Some(r) = recipe {
                    if input.contains(&r.input) && !output.would_overflow(&r.output) {
                        input.remove(&r.input);
                        self.status = MachineStatus::Processing {
                            recipe: r,
                            elapsed_ticks: 1,
                        };
                    }
                }
            }
            MachineStatus::Processing { elapsed_ticks, .. } => {
                *elapsed_ticks += 1;
            }
        }

        match &mut self.status {
            MachineStatus::Idle => {}
            MachineStatus::Processing {
                recipe,
                elapsed_ticks,
            } => {
                if *elapsed_ticks >= recipe.ticks {
                    output.insert(&recipe.output);
                    self.status = MachineStatus::Idle;
                }
            }
        }
    }

    pub fn allowed_items(&self, input: &InputBuffer) -> Filter {
        match &self.configured_recipe {
            Some(r) => {
                let items: Vec<Item> = r
                    .input
                    .iter()
                    .filter_map(|stack| {
                        let buffer_count = input
                            .slots
                            .iter()
                            .find(|s| s.item == stack.item)
                            .map(|s| s.count as u32)
                            .unwrap_or(0);
                        if buffer_count < stack.count as u32 * 2 {
                            Some(stack.item)
                        } else {
                            None
                        }
                    })
                    .collect();
                Filter::from_iter(items)
            }
            None => Filter::none(),
        }
    }
}

impl WorldDrop for Assembler {
    fn drop_items(&self) -> Vec<Stack> {
        let mut drops = vec![Stack::from(Item::Assembler)];
        if let MachineStatus::Processing { recipe, .. } = &self.status {
            drops.extend(recipe.input.iter().cloned());
        }
        drops
    }
}

#[derive(Component, Debug, PartialEq, Clone)]
pub struct Filter(HashSet<Item>);

impl Filter {
    pub fn accepts(&self, item: Item) -> bool {
        self.0.contains(&item)
    }

    pub fn none() -> Self {
        Self(HashSet::new())
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self::none()
    }
}

impl FromIterator<Item> for Filter {
    fn from_iter<T: IntoIterator<Item = Item>>(iter: T) -> Self {
        Self(HashSet::from_iter(iter))
    }
}

impl From<Recipe> for Filter {
    fn from(value: Recipe) -> Self {
        match value {
            Recipe::FurnaceRecipe(fr) => Self(HashSet::from_iter([fr.input.item])),
            Recipe::AssemblerRecipe(ar) => Self(ar.input.iter().map(|s| s.item).collect()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CollectorState {
    ReadyToPickUp,
    MovingItem {
        item: Item,
        visual: Entity,
        start: Vec3,
        end: Vec3,
        ticks: u32,
        /// True on the first tick after pickup — the system will trigger `PlaceItem`
        /// so the UI plugin can attach a model. Cleared after the first tick to
        /// prevent triggering `PlaceItem` on an entity that already has visuals.
        needs_place_item: bool,
    },
    ReadyToDropOff {
        item: Item,
        visual: Entity,
    },
    MovingToStart {
        ticks: u32,
    },
}

#[derive(Component)]
pub struct Collector {
    pub state: CollectorState,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            state: CollectorState::ReadyToPickUp,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn buffer_accepts_items() {
        let mut buffer = Buffer::default();
        buffer.insert(&[Item::Belt.into()]);
        let expected = Buffer {
            slots: vec![Stack {
                item: Item::Belt,
                count: 1,
            }],
        };
        assert_eq!(buffer, expected);
    }

    #[test]
    fn buffer_combines_like_items() {
        let mut buffer = Buffer::default();
        buffer.insert(&[Item::Belt.into()]);
        buffer.insert(&[Item::Belt.into()]);
        let expected = Buffer {
            slots: vec![Stack {
                item: Item::Belt,
                count: 2,
            }],
        };
        assert_eq!(buffer, expected);
    }

    #[test]
    fn assembler_instant_craft() {
        let recipe = AssemblerRecipe {
            input: vec![Item::Source.into()],
            output: vec![Item::Sink.into()],
            ticks: 1,
        };
        let mut input = InputBuffer::default();
        input.insert(&recipe.input);
        let mut output = OutputBuffer::default();
        let mut assem = Assembler::default();
        assem.configured_recipe = Some(recipe.clone());

        assem.tick(&mut input, &mut output);

        assert_eq!(input, InputBuffer::default());
        assert_eq!(output, {
            let mut output = OutputBuffer::default();
            output.insert(&recipe.output);
            output
        });
    }

    #[test]
    fn assembler_missing_recipe() {
        let recipe = AssemblerRecipe {
            input: vec![Item::Source.into()],
            output: vec![Item::Sink.into()],
            ticks: 1,
        };
        let mut input = InputBuffer::default();
        input.insert(&recipe.input);
        let mut output = OutputBuffer::default();
        let mut assem = Assembler::default();

        assem.tick(&mut input, &mut output);

        assert_eq!(input, {
            let mut buffer = InputBuffer::default();
            buffer.insert(&recipe.input);
            buffer
        });
        assert_eq!(output, OutputBuffer::default());
    }

    #[test]
    fn assembler_missing_input() {
        let recipe = AssemblerRecipe {
            input: vec![Item::Source.into()],
            output: vec![Item::Sink.into()],
            ticks: 1,
        };
        let mut input = InputBuffer::default();
        let mut output = OutputBuffer::default();
        let mut assem = Assembler::default();
        assem.configured_recipe = Some(recipe.clone());

        assem.tick(&mut input, &mut output);

        assert_eq!(input, InputBuffer::default());
        assert_eq!(output, OutputBuffer::default());
    }

    #[test]
    fn assembler_delayed_craft() {
        let recipe = AssemblerRecipe {
            input: vec![Item::Source.into()],
            output: vec![Item::Sink.into()],
            ticks: 2,
        };
        let mut input = InputBuffer::default();
        input.insert(&recipe.input);
        let mut output = OutputBuffer::default();
        let mut assem = Assembler::default();
        assem.configured_recipe = Some(recipe.clone());

        assem.tick(&mut input, &mut output);
        assert_eq!(input, InputBuffer::default());
        assert_eq!(output, OutputBuffer::default());

        assem.tick(&mut input, &mut output);
        assert_eq!(input, InputBuffer::default());
        assert_eq!(output, {
            let mut output = OutputBuffer::default();
            output.insert(&recipe.output);
            output
        });
    }

    #[test]
    fn filter_for_empty_assembler() {
        let a = Assembler::default();
        let actual = a.allowed_items(&InputBuffer::default());
        let expected = Filter::none();
        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_for_configured_assembler() {
        let recipe = AssemblerRecipe {
            input: vec![Item::Source.into()],
            output: vec![Item::Sink.into()],
            ticks: 2,
        };
        let mut a = Assembler::default();
        a.configured_recipe = Some(recipe.clone());

        let actual = a.allowed_items(&InputBuffer::default());
        let expected = Filter::from_iter([Item::Source]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_for_configured_assembler_with_items() {
        let recipe = AssemblerRecipe {
            input: vec![Item::Source.into(), Item::IronOre.into()],
            output: vec![Item::Sink.into()],
            ticks: 2,
        };
        let mut a = Assembler::default();
        a.configured_recipe = Some(recipe.clone());

        let mut buffer = InputBuffer::default();
        buffer.insert(&[Stack {
            item: Item::Source,
            count: 2,
        }]);
        let actual = a.allowed_items(&buffer);
        let expected = Filter::from_iter([Item::IronOre]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_all_inputs_fully_stocked() {
        let recipe = AssemblerRecipe {
            input: vec![Item::Source.into(), Item::IronOre.into()],
            output: vec![Item::Sink.into()],
            ticks: 1,
        };
        let mut a = Assembler::default();
        a.configured_recipe = Some(recipe.clone());

        let mut buffer = InputBuffer::default();
        buffer.insert(&[
            Stack {
                item: Item::Source,
                count: 2,
            },
            Stack {
                item: Item::IronOre,
                count: 2,
            },
        ]);
        let actual = a.allowed_items(&buffer);
        let expected = Filter::none();
        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_allow_items_for_two_crafts() {
        let recipe = AssemblerRecipe {
            input: vec![
                Stack {
                    item: Item::Source,
                    count: 1,
                },
                Stack {
                    item: Item::IronOre,
                    count: 2,
                },
                Stack {
                    item: Item::Belt,
                    count: 3,
                },
            ],
            output: vec![Item::Sink.into()],
            ticks: 2,
        };
        let mut a = Assembler::default();
        a.configured_recipe = Some(recipe.clone());

        let mut buffer = InputBuffer::default();
        buffer.insert(&[
            Stack {
                item: Item::Source,
                count: 2,
            },
            Stack {
                item: Item::IronOre,
                count: 2,
            },
        ]);
        let actual = a.allowed_items(&buffer);
        let expected = Filter::from_iter([Item::IronOre, Item::Belt]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn furnace_instant_craft() {
        let recipe = FurnaceRecipe {
            input: Item::IronOre.into(),
            output: Item::IronIngot.into(),
            ticks: 1,
        };
        let mut input = InputBuffer::default();
        input.insert(&[recipe.input]);
        let mut output = OutputBuffer::default();
        let mut furnace = Furnace::default();

        furnace.tick(&mut input, &mut output, &[recipe]);

        assert_eq!(input, InputBuffer::default());
        assert_eq!(output, {
            let mut output = OutputBuffer::default();
            output.insert(&[recipe.output]);
            output
        });
    }

    #[test]
    fn furnace_missing_recipe() {
        let recipe = FurnaceRecipe {
            input: Item::IronOre.into(),
            output: Item::IronIngot.into(),
            ticks: 1,
        };
        let mut input = InputBuffer::default();
        input.insert(&[recipe.input]);
        let mut output = OutputBuffer::default();
        let mut furnace = Furnace::default();

        furnace.tick(&mut input, &mut output, &[]);

        assert_eq!(input, {
            let mut buffer = InputBuffer::default();
            buffer.insert(&[recipe.input]);
            buffer
        });
        assert_eq!(output, OutputBuffer::default());
    }

    #[test]
    fn furnace_missing_input() {
        let recipe = FurnaceRecipe {
            input: Item::IronOre.into(),
            output: Item::IronIngot.into(),
            ticks: 1,
        };
        let mut input = InputBuffer::default();
        let mut output = OutputBuffer::default();
        let mut furnace = Furnace::default();

        furnace.tick(&mut input, &mut output, &[recipe]);

        assert_eq!(input, InputBuffer::default());
        assert_eq!(output, OutputBuffer::default());
    }

    #[test]
    fn furnace_delayed_craft() {
        let recipe = FurnaceRecipe {
            input: Item::IronOre.into(),
            output: Item::IronIngot.into(),
            ticks: 2,
        };
        let mut input = InputBuffer::default();
        input.insert(&[recipe.input]);
        let mut output = OutputBuffer::default();
        let mut furnace = Furnace::default();

        furnace.tick(&mut input, &mut output, &[recipe]);
        assert_eq!(input, InputBuffer::default());
        assert_eq!(output, OutputBuffer::default());

        furnace.tick(&mut input, &mut output, &[recipe]);
        assert_eq!(input, InputBuffer::default());
        assert_eq!(output, {
            let mut output = OutputBuffer::default();
            output.insert(&[recipe.output]);
            output
        });
    }

    #[test]
    fn filter_for_empty_furnace_no_recipes() {
        let a = Furnace::default();

        let actual = a.allowed_items(&InputBuffer::default(), &[]);

        let expected = Filter::none();
        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_for_empty_furnace_one_recipe() {
        let a = Furnace::default();

        let actual = a.allowed_items(
            &InputBuffer::default(),
            &[FurnaceRecipe {
                input: Item::IronOre.into(),
                output: Item::IronIngot.into(),
                ticks: 1,
            }],
        );

        let expected = Filter::from_iter([Item::IronOre]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_for_empty_furnace_two_recipe() {
        let a = Furnace::default();

        let actual = a.allowed_items(
            &InputBuffer::default(),
            &[
                FurnaceRecipe {
                    input: Item::IronOre.into(),
                    output: Item::IronIngot.into(),
                    ticks: 1,
                },
                FurnaceRecipe {
                    input: Item::Source.into(),
                    output: Item::Sink.into(),
                    ticks: 1,
                },
            ],
        );

        let expected = Filter::from_iter([Item::IronOre, Item::Source]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_for_empty_furnace_two_recipe_partial() {
        let a = Furnace::default();

        let mut input = InputBuffer::default();
        input.insert(&[Stack {
            item: Item::Source,
            count: 1,
        }]);
        let actual = a.allowed_items(
            &input,
            &[
                FurnaceRecipe {
                    input: Item::IronOre.into(),
                    output: Item::IronIngot.into(),
                    ticks: 1,
                },
                FurnaceRecipe {
                    input: Item::Source.into(),
                    output: Item::Sink.into(),
                    ticks: 1,
                },
            ],
        );

        let expected = Filter::from_iter([Item::Source]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_for_empty_furnace_two_recipe_meets_minimum() {
        let a = Furnace::default();

        let mut input = InputBuffer::default();
        input.insert(&[Stack {
            item: Item::Source,
            count: 2,
        }]);
        let actual = a.allowed_items(
            &input,
            &[
                FurnaceRecipe {
                    input: Item::IronOre.into(),
                    output: Item::IronIngot.into(),
                    ticks: 1,
                },
                FurnaceRecipe {
                    input: Item::Source.into(),
                    output: Item::Sink.into(),
                    ticks: 1,
                },
            ],
        );

        let expected = Filter::from_iter([]);
        assert_eq!(actual, expected);
    }
}
