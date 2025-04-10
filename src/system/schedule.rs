use crate::world::World;
use super::{IntoSystemConfigs, System, SystemConfig, SystemConfigs};
use std::{any::TypeId, collections::HashMap};

pub trait Phase: 'static {
    fn run(&self) {}

    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct PhaseId(u32);

pub struct Schedule {
    phases: Vec<Box<dyn Phase>>,
    configs: Vec<Vec<SystemConfig>>,
    phase_map: HashMap<TypeId, PhaseId>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            phases: vec![],
            configs: vec![],
            phase_map: HashMap::new(),
        }
    }

    pub fn add_phase(&mut self, phase: impl Phase) -> PhaseId {
        let ty = phase.type_id();
        match self.phase_map.get(&ty).copied() {
            Some(id) => {
                self.phases[id.0 as usize] = Box::new(phase) as Box<dyn Phase>;
                id
            }
            None => {
                let id = PhaseId(self.phases.len() as u32);
                self.phases.push(Box::new(phase));
                self.phase_map.insert(ty, id);
                self.configs.push(vec![]);
                id
            }
        }
    }

    pub fn add_configs<M>(&mut self, phase: impl Phase, configs: impl IntoSystemConfigs<M>) {
        let id = self.add_phase(phase);
        let phase_configs = &mut self.configs[id.0 as usize];

        match configs.configs() {
            SystemConfigs::Config(config) => phase_configs.push(config),
            SystemConfigs::Configs(configs) => phase_configs.extend(configs),
        }
    }

    pub fn build(self, world: &mut World) {}
}

pub struct PhaseNode {
    phase: Box<dyn Phase>,
    systems: Vec<System>,
    children: Vec<PhaseNode>,
}

pub struct Systems {
    
}
