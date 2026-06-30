//! Shared rendering for the contents of a slot.
//!
//! The outer slot container differs per view, so each view builds its own. The *contents* of a
//! filled slot — the item icon plus its count — are identical, and live here.

use bevy::prelude::*;
use common::inventory::Stack;
use gui::ItemExt;

/// Edge length, in pixels, of a single slot.
pub const SIZE: f64 = 64.0;

/// The icon-plus-count overlay shown inside a filled slot.
pub fn stack_content(stack: Stack) -> impl SceneList {
    bsn_list! {
        stack_icon(stack),
        item_count(stack),
    }
}

fn stack_icon(stack: Stack) -> impl Scene {
    bsn! {
        ImageNode {
            image: { stack.item.icon() },
            color: {
                if stack.count == 0 {
                    Color::linear_rgb(0.25, 0.25, 0.25)
                } else {
                    Color::WHITE
                }
            },
        }
        Node {
            width: percent(100),
            height: percent(100),
        }
    }
}

fn item_count(stack: Stack) -> impl Scene {
    if stack.count == 1 && stack.item.stack_size() == 1 {
        Box::new(bsn!()) as Box<dyn Scene>
    } else {
        Box::new(bsn!(
            Text::new(format!("{}", stack.count))
            Node {
                position_type: PositionType::Absolute,
                bottom: px(2.0),
                right: px(4.0),
            }
        ))
    }
}
