#[allow(dead_code)]
mod config;

fn main() {
    println!("Hello, world!");
    config::root::example_root();
    config::bundle::example_bundle();
    let root_config = config::Config::default();
    print!("Root config: {:#?}", root_config);
}
