#[allow(dead_code, unused_macros)]
mod config;
use kdl;
use miette;

fn main() -> miette::Result<()> {
    let mut root_config = config::Config::default();
    let kdl_str = r#"
    // env GIBRISH inherit="true" // error: invalid type
    // env GIBRISH inherit=(time)"true" // error: invalid type
    // env HOME inherit="true" // error: invalid type
    env GIBRISH_1 inherit=#false
    env PLAIN inherit=#false
    env KEY1 "value1"
    // env #true "value"
    env (time)"10:20" "value"
    env KEY2 "${KEY1}_suffix"
    env KEY3="${KEY2}_more"
    env KEY4="${GIBRISH}_extended"
    env KEY5 "${GIBRISH_1:-not-inherited}_extended"
    env KEY6 "${PLAIN:-removed}_extended"
    // hello "ignored"
    "#;
    let kdl_doc: kdl::KdlDocument = kdl_str.parse()?;
    root_config
        .env
        .apply_kdl(&kdl_doc)
        .map_err(|error| miette::Error::new(error).with_source_code(kdl_str))?;
    print!("Updated config: {:#?}", root_config);

    Ok(())
}
