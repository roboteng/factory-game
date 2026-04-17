# Overview

It should seem to the player that items have stack sizes, and take up space.

The main effect of this is two-fold:
1. Provide inventory pressure on the player
2. Provide backpressure in machines, so they don't run forever, if not managed well.

# Details

Stack size are defined by the `Item::stack_size` method.

Machines should continue to produce items until a single stack in the output is full.
For some things, this is easy to consider, since they only have 1 output type of item.
It is possible for a machine to produce multiple kinds of items. In this case, if any item would go above the stack size, the machine stops and the result is effectevly backpressure.
This assumes that we know what the machine will produce before we start. In general, this is true, but may not be true for things more on the nature side.

## Rules
When placing things in the players inventory, the count of a stack should never exceed the stack size.
When this would happen, create an additional stack of that same item.
So, if a player has 256 dirt, and dirt stacks to 64, then they'll have 4 stacks of dirt
The player can have the ability to split stacks of items.
The player can have the ability to combine stacks of items. We need to follow the stack size rule when combine items, to make sure we never exceed the limit.

## Exceptions
### Machine inputs/outputs
For machines, it is more important to group like items together, rather than enforce stack sizes.
The filters on machines, by default, will be configured to keep twice the recipe amount of items in input.
If a recipe calls for a stack of items, then its expected to have twice the number of items in that stack.

## Edge cases
- When a recipe produces more than one stack of items
  - Each item type should get its own stack, and we'll ignore stack limits. The next craft won't start until the output buffer is empty.
