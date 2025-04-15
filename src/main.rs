use app::App;
use system::{
    query::{Added, Query},
    schedule::Phase,
};
use world::{Component, Spawner};

pub mod app;
pub mod core;
pub mod system;
pub mod world;

pub struct Start;
impl Phase for Start {}

pub struct Update;
impl Phase for Update {}

fn main() {
    App::new()
        .register::<Name>()
        .register::<Age>()
        .add_systems(Start, |mut spawner: Spawner| {
            println!("Spawning entity");
            spawner
                .spawn()
                .with(Name("Alice".to_string()))
                .with(Age(30))
                .finish();
        })
        .add_systems(Update, |query: Query<(&Age, &Name), Added<Age>>| {
            println!("Running query");
            for (age, name) in query.iter() {
                println!("Query found age: {:?}", age);
                println!("Query found name: {:?}", name);
            }
        })
        .build()
        .run(Start)
        .run(Update);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Age(u32);
impl Component for Age {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);
impl Component for Name {}
