use app::App;
use system::{
    query::{Added, Query, QueryState},
    schedule::Phase,
};
use world::Component;

pub mod app;
pub mod core;
pub mod system;
pub mod world;

pub struct Update;
impl Phase for Update {}

fn main() {
    let mut world = world::World::new();
    world.register::<Name>();
    world.register::<Age>();

    let entity = world.spawn();
    world.add_component(entity, Age(30));
    world.add_component(entity, Name("Alice".to_string()));

    let state = QueryState::new(&world);
    let query = Query::<(&Age, &Name), Added<Age>>::new(&world, &state);
    for (age, name) in query.iter() {
        println!("Query found age: {:?}", age);
        println!("Query found name: {:?}", name);
    }

    App::new().build().run(Update);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Age(u32);
impl Component for Age {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);
impl Component for Name {}
