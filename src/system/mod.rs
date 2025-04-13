use crate::{
    core::{AccessBitset, Frame, SparseIndex},
    world::{ComponentId, ResourceId, World, cell::WorldCell},
};
use std::{any::Any, borrow::Cow, collections::HashSet};

pub mod arg;
pub mod executor;
pub mod query;
pub mod schedule;

pub type SystemName = Cow<'static, str>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemId(u32);
impl SystemId {
    fn new() -> Self {
        static mut ID: u32 = 0;
        unsafe {
            let id = ID;
            ID += 1;
            SystemId(id)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemAccess {
    Component { id: ComponentId, access: Access },
    Resource { id: ResourceId, access: Access },
}

impl SystemAccess {
    pub fn resource(id: ResourceId, access: Access) -> Self {
        SystemAccess::Resource { id, access }
    }

    pub fn component(id: ComponentId, access: Access) -> Self {
        SystemAccess::Component { id, access }
    }
}

pub struct SystemMeta {
    pub id: SystemId,
    pub name: Option<SystemName>,
    /// Components that the system accesses.
    pub components: AccessBitset,
    /// Resources that the system accesses.
    pub resources: AccessBitset,
    /// The system contains only send resources.
    pub send: bool,
    /// The system should be ran exclusively in the given frame.
    pub exclusive: bool,
    /// The frame in which the system was last executed.
    pub frame: Frame,
}

pub struct SystemConfig {
    id: SystemId,
    name: Option<SystemName>,
    exclusive: bool,
    send: bool,
    dependencies: HashSet<SystemId>,
    init: fn(&mut World) -> Box<dyn Any + Send + Sync>,
    access: fn(&Box<dyn Any + Send + Sync>) -> Vec<SystemAccess>,
    execute: SystemExecute,
}

impl SystemConfig {
    pub fn into_system_node(self, world: &mut World) -> SystemNode {
        let state = (self.init)(world);
        let mut components = AccessBitset::with_capacity(world.components().len());
        let mut resources = AccessBitset::with_capacity(world.resources().len());

        for access in (self.access)(&state) {
            match access {
                SystemAccess::Component { id, access } => match access {
                    Access::Read => components.read(id.to_usize()),
                    Access::Write => components.write(id.to_usize()),
                },
                SystemAccess::Resource { id, access } => match access {
                    Access::Read => resources.read(id.to_usize()),
                    Access::Write => resources.write(id.to_usize()),
                },
            };
        }

        let meta = SystemMeta {
            id: self.id,
            name: self.name,
            components,
            resources,
            send: self.send,
            exclusive: self.exclusive,
            frame: Frame::ZERO,
        };

        SystemNode {
            meta,
            state,
            execute: self.execute,
            dependencies: self.dependencies,
        }
    }
}

pub struct SystemNode {
    pub meta: SystemMeta,
    pub state: Box<dyn Any + Send + Sync>,
    pub execute: Box<dyn Fn(&Box<dyn Any + Send + Sync>, WorldCell, &SystemMeta) + Send + Sync>,
    pub dependencies: HashSet<SystemId>,
}

impl SystemNode {
    pub fn has_dependency(&self, other: &SystemNode) -> bool {
        self.dependencies.contains(&other.meta.id)
            || self.meta.components.conflicts(&other.meta.components)
            || self.meta.resources.conflicts(&other.meta.resources)
    }
}

pub enum SystemConfigs {
    Config(SystemConfig),
    Configs(Vec<SystemConfig>),
}

impl SystemConfigs {
    pub fn single(self) -> SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => configs.into_iter().next().unwrap(),
        }
    }
}

impl AsRef<SystemConfig> for SystemConfigs {
    fn as_ref(&self) -> &SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => &configs[0],
        }
    }
}

impl AsMut<SystemConfig> for SystemConfigs {
    fn as_mut(&mut self) -> &mut SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => &mut configs[0],
        }
    }
}

impl SystemConfigs {
    pub fn new(config: SystemConfig) -> Self {
        SystemConfigs::Config(config)
    }

    pub fn configs(configs: Vec<SystemConfig>) -> Self {
        SystemConfigs::Configs(configs)
    }

    pub fn config(&self) -> &SystemConfig {
        match self {
            SystemConfigs::Config(config) => config,
            SystemConfigs::Configs(configs) => &configs[0],
        }
    }
}

pub trait IntoSystemConfigs<M> {
    fn configs(self) -> SystemConfigs;
    fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs;
    fn after<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs
    where
        Self: Sized,
    {
        configs.before(self)
    }
}

impl IntoSystemConfigs<()> for SystemConfigs {
    fn configs(self) -> SystemConfigs {
        self
    }

    fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
        let configs = configs.configs();

        match (self, configs) {
            (SystemConfigs::Config(before), SystemConfigs::Config(mut after)) => {
                after.dependencies.insert(before.id);
                Self::Configs(vec![before, after])
            }
            (SystemConfigs::Config(before), SystemConfigs::Configs(mut after)) => {
                after.iter_mut().for_each(|s| {
                    s.dependencies.insert(before.id);
                });
                after.insert(0, before);
                Self::Configs(after)
            }
            (SystemConfigs::Configs(mut before), SystemConfigs::Config(mut after)) => {
                after.dependencies.extend(before.iter().map(|s| s.id));
                before.push(after);
                Self::Configs(before)
            }
            (SystemConfigs::Configs(mut before), SystemConfigs::Configs(mut after)) => {
                after
                    .iter_mut()
                    .for_each(|s| s.dependencies.extend(before.iter().map(|s| s.id)));
                before.extend(after);
                Self::Configs(before)
            }
        }
    }
}

pub type SystemState = Box<dyn Any + Send + Sync>;
pub type SystemExecute =
    Box<dyn Fn(&Box<dyn Any + Send + Sync>, WorldCell, &SystemMeta) + Send + Sync>;

pub struct System {
    meta: SystemMeta,
    state: SystemState,
    execute: SystemExecute,
}

impl System {
    pub fn new(meta: SystemMeta, state: SystemState, execute: SystemExecute) -> Self {
        Self {
            meta,
            state,
            execute,
        }
    }

    pub fn execute(&self, world: WorldCell) {
        (self.execute)(&self.state, world, &self.meta);
    }
}

impl From<SystemNode> for System {
    fn from(value: SystemNode) -> Self {
        Self {
            meta: value.meta,
            state: value.state,
            execute: value.execute,
        }
    }
}
