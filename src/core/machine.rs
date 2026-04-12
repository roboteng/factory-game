use bevy::platform::collections::HashSet;

use super::*;
use crate::core::inventory::Stack;
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
                self.slots.push(stack.clone());
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
            self.slots
                .iter_mut()
                .find(|s| s.item == stack.item)
                .map(|s| s.count -= stack.count);
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

#[derive(Component, Default)]
pub struct Furnace {
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
                    input.remove(&[recipe.input]);
                    self.status = MachineStatus::Processing {
                        recipe: *recipe,
                        elapsed_ticks: 1,
                    };
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
}

#[derive(Component, Default)]
pub struct Assembler {
    pub status: MachineStatus<AssemblerRecipe>,
    pub configured_recipe: Option<AssemblerRecipe>,
}

impl Assembler {
    pub fn tick(&mut self, input: &mut InputBuffer, output: &mut OutputBuffer) {
        let recipe = self.configured_recipe.clone();
        match &mut self.status {
            MachineStatus::Idle => {
                if let Some(r) = recipe {
                    if input.contains(&r.input) {
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

    pub fn allowed_items(&self) -> Filter {
        match &self.configured_recipe {
            Some(r) => Filter::from_iter(r.input.iter().map(|s| s.item)),
            None => Filter::none(),
        }
    }
}

#[derive(Component, Debug, PartialEq)]
pub struct Filter(HashSet<Item>);

impl Filter {
    pub fn accepts(&self, item: Item) -> bool {
        self.0.contains(&item)
    }

    pub fn none() -> Self {
        Self(HashSet::new())
    }

    pub fn for_method(method: ProcessingMethod, recipes: &[Recipe]) -> Self {
        let mut items: Vec<Item> = recipes
            .iter()
            .flat_map(|r| match (r, method) {
                (Recipe::FurnaceRecipe(fr), ProcessingMethod::Furnace) => vec![fr.input.item],
                (Recipe::AssemblerRecipe(ar), ProcessingMethod::Assembler) => {
                    ar.input.iter().map(|s| s.item).collect()
                }
                _ => vec![],
            })
            .collect();
        items.sort();
        items.dedup();
        Self(HashSet::from_iter(items))
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

#[derive(Debug, Clone, Copy)]
pub enum CollectorState {
    ReadyToPickUp,
    MovingItem {
        item: Item,
        visual: Entity,
        start: Vec3,
        end: Vec3,
        ticks: u32,
    },
    ReadyToDropOff {
        item: Item,
    },
    MovingToStart {
        ticks: u32,
    },
}

#[derive(Component)]
pub struct Collector {
    pub state: CollectorState,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn filter_for_emtpy_assembler() {
        let a = Assembler::default();
        let actual = a.allowed_items();
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

        let actual = a.allowed_items();
        let expected = Filter::from_iter([Item::Source]);
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
}
