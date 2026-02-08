use std::num::NonZeroU16;

use bevy::prelude::*;

use crate::core::{Item, ItemRegEntry, ItemRegistry};

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
    pub fn insert(&mut self, stack: Stack, reg: &ItemRegEntry) -> Result<(), InventoryAddError> {
        todo!()
    }

    /// Adding items at a specific location in the inventory
    /// for example, by clicking
    ///
    /// It gives any remaining leftover
    pub fn insert_at(
        &mut self,
        stack: Option<Stack>,
        slot: u16,
        reg: &ItemRegistry,
    ) -> Option<Stack> {
        todo!()
    }

    pub fn item_count(&self, item: Item) -> u16 {
        todo!()
    }

    /// Returns the number of items acutally taken from the inventory
    pub fn take_n_items(&mut self, n: NonZeroU16, item: Item) -> u16 {
        todo!()
    }
}

pub enum InventoryAddError {
    TooFull,
}

pub struct Stack {
    item: Item,
    count: NonZeroU16,
}

#[cfg(test)]
mod tests {
    #[test]
    fn foobar() {}
}
