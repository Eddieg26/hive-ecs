use super::{GraphInfo, SystemExecutor};
use crate::system::System;

pub struct SequentialExecutor {
    systems: Box<[System]>,
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
        let mut stack = vec![];

        for (index, &dep_count) in dependencies.iter().enumerate() {
            if dep_count == 0 {
                stack.push(index);
            }
        }

        while let Some(node_index) = stack.pop() {
            order.push(node_index);

            for dependent in dependents[node_index].ones() {
                dependencies[dependent] -= 1;
                if dependencies[dependent] == 0 {
                    stack.push(dependent);
                }
            }
        }

        if order.len() != nodes.len() {
            panic!("Cyclic dependency detected in the system graph!");
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
