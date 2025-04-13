use super::{GraphInfo, SystemExecutor};
use crate::system::System;
use fixedbitset::FixedBitSet;

pub struct SequentialExecutor {
    systems: Vec<System>,
    order: Box<[usize]>,
}

impl SequentialExecutor {
    pub fn new(info: GraphInfo) -> Self {
        let GraphInfo {
            mut nodes,
            dependents,
            mut dependencies,
        } = info;

        let mut order = vec![];
        let mut visited = FixedBitSet::with_capacity(nodes.len());
        let mut stack = vec![];
        for index in 0..nodes.len() {
            stack.push(index);
            while let Some(node_index) = stack.pop() {
                if visited[node_index] {
                    continue;
                }

                if dependencies[node_index] == 0 {
                    visited.set(node_index, true);
                    order.push(node_index);

                    for dependent in dependents[node_index].ones() {
                        dependencies[dependent] -= 1;
                        stack.push(dependent);
                    }
                }
            }
        }

        Self {
            systems: nodes.drain(..).map(System::from).collect(),
            order: order.into_boxed_slice(),
        }
    }
}

impl SystemExecutor for SequentialExecutor {
    fn execute(&self, world: crate::world::WorldCell) {
        for index in &self.order {
            let system = &self.systems[*index];
            system.run(world);
        }
    }
}
