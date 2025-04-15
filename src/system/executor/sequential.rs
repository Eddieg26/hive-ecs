use super::SystemExecutor;
use crate::{
    core::{DagValues, IndexDag},
    system::System,
};

pub struct SequentialExecutor {
    systems: Box<[System]>,
    order: Box<[usize]>,
}

impl SequentialExecutor {
    pub fn new(systems: IndexDag<System>) -> Self {
        let DagValues {
            nodes, topology, ..
        } = systems.into_values();

        Self {
            systems: nodes.into_boxed_slice(),
            order: topology.into_boxed_slice(),
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
