use bevy::prelude::*;

use crate::core::Item;

#[derive(Component, Debug, PartialEq)]
pub struct Inventory(Vec<Option<Stack>>);

impl Inventory {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn get(&self, slot: u16) -> Option<Stack> {
        match self.0.get(slot as usize) {
            Some(Some(s)) => Some(*s),
            Some(None) => None,
            None => None,
        }
    }

    pub fn take_slot(&mut self, slot: u16) -> Option<Stack> {
        match self.0.get_mut(slot as usize) {
            Some(entry) => entry.take(),
            None => None,
        }
    }

    /// Add items to the first available slot
    pub fn insert(&mut self, stack: Stack) -> Result<(), InventoryAddError> {
        for slot in self.0.iter_mut() {
            match slot {
                None => {
                    *slot = Some(stack);
                    return Ok(());
                }
                Some(s) => {
                    if s.item == stack.item {
                        s.count += stack.count;
                        return Ok(());
                    }
                }
            }
        }
        self.0.push(Some(stack));
        Ok(())
    }

    /// Adding items at a specific location in the inventory
    /// for example, by clicking
    ///
    /// It gives any remaining leftover
    #[expect(unused)]
    pub fn insert_at(&mut self, stack: Option<Stack>, slot: u16) -> Option<Stack> {
        todo!()
    }

    pub fn item_count(&self, item: Item) -> u16 {
        self.0
            .iter()
            .filter_map(|slot| slot.as_ref())
            .filter(|stack| stack.item == item)
            .map(|stack| stack.count)
            .sum()
    }

    /// Returns the number of items actually taken from the inventory
    pub fn take_items(&mut self, item: Stack) -> u16 {
        let mut remaining = item.count;
        for slot in self.0.iter_mut() {
            if remaining == 0 {
                break;
            }
            if let Some(s) = slot {
                if s.item == item.item {
                    let take = remaining.min(s.count);
                    s.count -= take;
                    remaining -= take;
                    if s.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
        while matches!(self.0.last(), Some(None)) {
            self.0.pop();
        }
        item.count - remaining
    }
}

#[derive(Debug)]
pub enum InventoryAddError {
    #[expect(dead_code)]
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Stack {
    pub item: Item,
    pub count: u16,
}

impl Stack {
    pub fn new(item: Item, count: u16) -> Self {
        Self { item, count }
    }
}

impl From<Item> for Stack {
    fn from(item: Item) -> Self {
        Self { item, count: 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_is_empty() {
        let inv = Inventory::new();
        assert_eq!(inv.get(0), None);
    }

    #[test]
    fn inventory_accepts_items() {
        let mut inv = Inventory::new();
        let stack = Stack {
            item: Item::Dirt,
            count: 42.try_into().unwrap(),
        };
        inv.insert(stack).unwrap();
        assert_eq!(inv.get(0), Some(stack));
    }

    #[test]
    fn inventory_gives_items() {
        let mut inv = Inventory::new();
        let stack = Stack {
            item: Item::Dirt,
            count: 42.try_into().unwrap(),
        };
        inv.insert(stack).unwrap();
        let slot = inv.take_slot(0);
        assert_eq!(inv.get(0), None);
        assert_eq!(slot, Some(stack));
    }

    #[test]
    fn inventory_accepts_items_again() {
        let mut inv = Inventory::new();
        let stack = Stack {
            item: Item::Dirt,
            count: 42.try_into().unwrap(),
        };
        inv.insert(stack).unwrap();
        inv.take_slot(0);
        inv.insert(stack).unwrap();
        assert_eq!(inv.get(0), Some(stack));
    }

    #[test]
    fn inventory_combines_when_accepting() {
        let mut inv = Inventory::new();
        let stack = Stack {
            item: Item::Dirt,
            count: 42.try_into().unwrap(),
        };
        inv.insert(stack).unwrap();
        inv.insert(stack).unwrap();
        assert_eq!(
            inv.get(0),
            Some(Stack {
                item: Item::Dirt,
                count: 84.try_into().unwrap(),
            })
        );
    }

    #[test]
    fn accepts_two_items() {
        let mut inv = Inventory::new();

        inv.insert(Stack {
            item: Item::Rock,
            count: 1,
        })
        .unwrap();
        let stack = Stack {
            item: Item::Dirt,
            count: 42,
        };
        inv.insert(stack).unwrap();
        assert_eq!(inv.get(1), Some(stack));
    }

    #[test]
    fn take_items_from_empty() {
        let mut inv = Inventory::new();
        let actual = inv.take_items(Stack {
            item: Item::Belt,
            count: 1,
        });
        assert_eq!(inv, Inventory::new());
        assert_eq!(actual, 0);
    }

    #[test]
    fn take_items_all() {
        let mut inv = Inventory::new();
        inv.insert(Stack {
            item: Item::Belt,
            count: 1,
        })
        .unwrap();
        let actual = inv.take_items(Stack {
            item: Item::Belt,
            count: 1,
        });
        assert_eq!(inv, Inventory::new());
        assert_eq!(actual, 1);
    }

    #[test]
    fn take_items_partial() {
        let mut inv = Inventory::new();
        inv.insert(Stack {
            item: Item::Belt,
            count: 2,
        })
        .unwrap();
        let actual = inv.take_items(Stack {
            item: Item::Belt,
            count: 1,
        });
        assert_eq!(inv, {
            let mut inv = Inventory::new();
            inv.insert(Stack {
                item: Item::Belt,
                count: 1,
            })
            .unwrap();
            inv
        });
        assert_eq!(actual, 1);
    }
}
