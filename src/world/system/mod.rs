use crate::{
    core::{
        ComponentId, Entities, Frame, NonSend, NonSendMut, Res, ResMut, Resource, ResourceId,
        bitset::Bitset,
    },
    world::{World, cell::WorldCell},
};
use std::{any::Any, borrow::Cow, sync::Arc};

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
    pub components: Bitset,
    /// Resources that the system accesses.
    pub resources: Bitset,
    /// The system contains only send resources.
    pub send: bool,
    /// The system should be ran exclusively in the given frame.
    pub exclusive: bool,
    /// The frame in which the system is executed.
    pub frame: Frame,
}

pub struct SystemConfig {
    id: SystemId,
    name: Option<SystemName>,
    exclusive: bool,
    send: bool,
    dependencies: Vec<SystemId>,
    init: fn(&mut World) -> Box<dyn Any + Send + Sync>,
    access: fn(&Box<dyn Any + Send + Sync>) -> Vec<SystemAccess>,
    execute: Arc<dyn Fn(&mut Box<dyn Any + Send + Sync>, WorldCell, &SystemMeta)>,
}

impl SystemConfig {
    pub fn into_system_node(self, world: &mut World) -> SystemNode {
        let state = (self.init)(world);
        let access = (self.access)(&state);

        let meta = SystemMeta {
            id: self.id,
            name: self.name,
            components: Bitset::new(),
            resources: Bitset::new(),
            send: self.send,
            exclusive: self.exclusive,
            frame: world.frame().previous(),
        };

        let system = System {
            meta,
            state,
            execute: self.execute,
        };

        SystemNode {
            system,
            dependencies: self.dependencies,
            access,
        }
    }
}

pub struct SystemNode {
    pub(super) system: System,
    pub(super) dependencies: Vec<SystemId>,
    pub(super) access: Vec<SystemAccess>,
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
                after.dependencies.push(before.id);
                Self::Configs(vec![before, after])
            }
            (SystemConfigs::Config(before), SystemConfigs::Configs(mut after)) => {
                after
                    .iter_mut()
                    .for_each(|s| s.dependencies.push(before.id));
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

pub struct System {
    meta: SystemMeta,
    state: Box<dyn Any + Send + Sync>,
    execute: Arc<dyn Fn(&mut Box<dyn Any + Send + Sync>, WorldCell, &SystemMeta)>,
}

impl System {
    pub fn execute(&mut self, world: WorldCell) {
        (self.execute)(&mut self.state, world, &self.meta);
    }
}

#[allow(unused_variables)]
pub unsafe trait SystemArg: Sized {
    type Item<'world, 'state>: SystemArg<State = Self::State>;

    type State: Send + Sync + 'static;

    fn init(world: &mut World) -> Self::State;

    /// Validates that the argument can be accessed by the system
    unsafe fn validate(state: &Self::State, world: WorldCell, system: &SystemMeta) -> bool {
        true
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &SystemMeta,
    ) -> Self::Item<'world, 'state>;

    fn exclusive() -> bool {
        false
    }

    fn send() -> bool {
        true
    }

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![]
    }
}

pub type ArgItem<'world, 'state, A> = <A as SystemArg>::Item<'world, 'state>;

unsafe impl SystemArg for () {
    type Item<'world, 'state> = ();

    type State = ();

    fn init(_: &mut World) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        _world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        ()
    }
}

unsafe impl SystemArg for &World {
    type Item<'world, 'state> = &'world World;

    type State = ();

    fn init(_: &mut World) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get() }
    }

    fn exclusive() -> bool {
        true
    }
}

unsafe impl SystemArg for &Entities {
    type Item<'world, 'state> = &'world Entities;

    type State = ();

    fn init(_: &mut World) -> Self::State {
        ()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get().entities() }
    }
}

unsafe impl<R: Resource + Send> SystemArg for Res<'_, R> {
    type Item<'world, 'state> = Res<'world, R>;

    type State = ResourceId;

    fn init(world: &mut World) -> Self::State {
        world.register_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get().res() }
    }

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![SystemAccess::resource(*state, Access::Read)]
    }
}

unsafe impl<R: Resource + Send> SystemArg for ResMut<'_, R> {
    type Item<'world, 'state> = ResMut<'world, R>;

    type State = ResourceId;

    fn init(world: &mut World) -> Self::State {
        world.register_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get_mut().res_mut() }
    }

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![SystemAccess::resource(*state, Access::Write)]
    }
}

unsafe impl<R: Resource> SystemArg for NonSend<'_, R> {
    type Item<'world, 'state> = NonSend<'world, R>;

    type State = ResourceId;

    fn init(world: &mut World) -> Self::State {
        world.register_non_send_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get().non_send_res() }
    }

    fn send() -> bool {
        false
    }

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![SystemAccess::resource(*state, Access::Read)]
    }
}

unsafe impl<R: Resource> SystemArg for NonSendMut<'_, R> {
    type Item<'world, 'state> = NonSendMut<'world, R>;

    type State = ResourceId;

    fn init(world: &mut World) -> Self::State {
        world.register_non_send_resource::<R>()
    }

    unsafe fn get<'world, 'state>(
        _state: &'state mut Self::State,
        mut world: WorldCell<'world>,
        _system: &SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { world.get_mut().non_send_res_mut() }
    }

    fn send() -> bool {
        false
    }

    fn access(state: &Self::State) -> Vec<SystemAccess> {
        vec![SystemAccess::resource(*state, Access::Write)]
    }
}

macro_rules! impl_into_system_configs {
    ($($arg:ident),*) => {
    #[allow(non_snake_case)]
    impl<F, $($arg: SystemArg),*> IntoSystemConfigs<(F, $($arg),*)> for F
        where
            for<'world, 'state> F: Fn($($arg),*) + Fn($(ArgItem<'world,'state, $arg>),*) + Send + Sync + 'static,
        {

            fn configs(self) -> SystemConfigs {
                let name = std::any::type_name::<F>();

                let init = |world: &mut World| {
                    let ($($arg,)*) = ($($arg::init(world),)*);
                    let state = ($($arg,)*);
                    Box::new(state) as Box<dyn Any + Send + Sync>
                };

                let execute = move |state: &mut Box<dyn Any + Send + Sync>, world: WorldCell, system: &SystemMeta| {
                    let ($($arg,)*) = state.downcast_mut::<($($arg::State,)*)>().unwrap();
                    let ($($arg,)*) = unsafe {($($arg::get($arg, world, system),)*)};

                    self($($arg,)*);
                };

                let access = |state: &Box<dyn Any + Send + Sync>| {
                    let ($($arg,)*) = state.downcast_ref::<($($arg::State,)*)>().unwrap();
                    let mut access = Vec::new();
                    $(access.extend($arg::access($arg));)*
                    access
                };

                let send = ($($arg::send() &&)* true);
                let exclusive = ($($arg::exclusive() ||)* false);

                SystemConfigs::Config(SystemConfig {
                    id: SystemId::new(),
                    name: Some(name.into()),
                    exclusive,
                    send,
                    dependencies: Vec::new(),
                    init,
                    execute: Arc::new(execute),
                    access
                })
            }

            fn before<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
                let before = self.configs().single();
                let after_configs = configs.configs();

                match after_configs {
                    SystemConfigs::Config(mut config) => {
                        config.dependencies.push(before.id);
                        SystemConfigs::Configs(vec![before, config])
                    }
                    SystemConfigs::Configs(mut configs) => {
                        configs.iter_mut().for_each(|config| {
                            config.dependencies.push(before.id);
                        });

                        configs.insert(0, before);
                        SystemConfigs::Configs(configs)
                    }
                }
            }

            fn after<Marker>(self, configs: impl IntoSystemConfigs<Marker>) -> SystemConfigs {
                let configs = configs.configs();
                configs.before(self)
            }
        }

        #[allow(non_snake_case)]
        unsafe impl<$($arg: SystemArg),*> SystemArg for ($($arg,)*) {
            type Item<'world, 'state> = ($($arg::Item<'world, 'state>,)*);
            type State = ($($arg::State,)*);

            fn init(world: &mut World) -> Self::State {
                let ($($arg,)*) = ($($arg::init(world),)*);
                ($($arg,)*)
            }

            unsafe fn get<'world, 'state>(state: &'state mut Self::State, world: WorldCell<'world>, system: &SystemMeta,) -> Self::Item<'world, 'state> {
                let ($($arg,)*) = state;
                let ($($arg,)*) = unsafe {($($arg::get($arg, world, system),)*)};
                ($($arg,)*)
            }

             fn exclusive() -> bool {
                ($($arg::exclusive() ||)* false)
            }

            fn send() -> bool {
                ($($arg::send() &&)* true)
            }

            fn access(state: &Self::State) -> Vec<SystemAccess> {
                let ($($arg,)*) = state;
                let mut access = Vec::new();
                $(access.extend($arg::access($arg));)*
                access
            }
        }
    };
}

impl_into_system_configs!(A);
impl_into_system_configs!(A, B);
impl_into_system_configs!(A, B, C);
impl_into_system_configs!(A, B, C, D);
impl_into_system_configs!(A, B, C, D, E);
impl_into_system_configs!(A, B, C, D, E, F2);
impl_into_system_configs!(A, B, C, D, E, F2, G);
impl_into_system_configs!(A, B, C, D, E, F2, G, H);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I);
impl_into_system_configs!(A, B, C, D, E, F2, G, H, I, J);
