use std::num::NonZeroU16;

use bevy::prelude::*;

use crate::core::Item;

#[derive(Component)]
pub struct Inventory(Vec<Option<Stack>>);

impl Inventory {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn get(&self, slot: u16) -> Option<&Stack> {
        match self.0.get(slot as usize) {
            Some(Some(s)) => Some(s),
            Some(None) => None,
            None => None,
        }
    }

    pub fn get_mut(&mut self, slot: u16) -> Option<&mut Stack> {
        match self.0.get_mut(slot as usize) {
            Some(Some(s)) => Some(s),
            Some(None) => None,
            None => None,
        }
    }

    /// Add items to the first availible slot
    pub fn insert(&mut self, stack: Stack) -> Result<(), InventoryAddError> {
        // TODO: combine stacks
        self.0.push(Some(stack));
        Ok(())
    }

    /// Adding items at a specific location in the inventory
    /// for example, by clicking
    ///
    /// It gives any remaining leftover
    pub fn insert_at(&mut self, stack: Option<Stack>, slot: u16) -> Option<Stack> {
        todo!()
    }

    pub fn item_count(&self, item: Item) -> u16 {
        self.0
            .iter()
            .filter_map(|slot| slot.as_ref())
            .filter(|stack| stack.item == item)
            .map(|stack| stack.count.get())
            .sum()
    }

    /// Returns the number of items acutally taken from the inventory
    pub fn take_n_items(&mut self, n: NonZeroU16, item: Item) -> u16 {
        todo!()
    }
}

#[derive(Debug)]
pub enum InventoryAddError {
    TooFull,
}
impl std::fmt::Display for InventoryAddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InventoryAddError::TooFull => f.write_str("Too Full"),
        }
    }
}

impl std::error::Error for InventoryAddError {}

pub struct Stack {
    pub item: Item,
    pub count: NonZeroU16,
}

impl Stack {
    pub fn new(item: Item, count: NonZeroU16) -> Self {
        Self { item, count }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn foobar() {}
}
