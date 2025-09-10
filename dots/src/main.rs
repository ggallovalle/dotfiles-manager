#[allow(dead_code)]

mod config;

fn main() {
    println!("Hello, world!");
    config::root::example_root();
    config::bundle::example_bundle();
    let root_config = config::RootConfig::default();
    let value = root_config.env_expand("$ZDOTDIR/home");
    println!("Expanded value: {}", value);
    print!("Root config: {:#?}", root_config);
}
