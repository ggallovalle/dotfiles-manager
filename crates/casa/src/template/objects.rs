use crate::env::Env;
use minijinja::value::{Enumerator, Object, ObjectExt, Value, from_args};

impl Object for Env {
    fn get_value(self: &std::sync::Arc<Self>, key: &Value) -> Option<Value> {
        dbg!(key);
        if let Some(key) = key.as_str() {
            if let Some(value) = self.get_str(key) {
                return Some(Value::from(value));
            }
        }
        None
    }

    fn enumerate(self: &std::sync::Arc<Self>) -> Enumerator {
        self.mapped_enumerator(|this| Box::new(this.keys().into_iter().map(Value::from)))
    }

    fn call_method(
        self: &std::sync::Arc<Self>,
        state: &minijinja::State<'_, '_>,
        method: &str,
        args: &[Value],
    ) -> Result<Value, minijinja::Error> {
        match method {
            "expand" => {
                let (source,): (&str,) = from_args(args)?;
                let value = self.expand(source).map_err(|e| {
                    minijinja::Error::from(minijinja::ErrorKind::InvalidOperation).with_source(e)
                })?;
                Ok(value.into())
            }
            _ => Err(minijinja::ErrorKind::UnknownMethod.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::{Environment, context};
    use pretty_assertions::{assert_eq, assert_ne};
    use std::sync::Arc;

    #[test]
    fn test_env_object() {
        let mut env = Env::empty();
        env.insert_simple("HOME", "/home/some-user");
        env.insert_simple("USER", "some-user");
        let env_value = Arc::new(env);
        let mut jinja = Environment::new();
        jinja.set_debug(true);
        jinja.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
        let render_eq = |template: &str, expected: &str| {
            let ctx = context!(env => Value::from_dyn_object(env_value.clone()));
            assert_eq!(jinja.render_str(template, ctx).unwrap(), expected);
        };
        render_eq("{{env.HOME}}/.config", "/home/some-user/.config");
        render_eq(r#"{{ env.expand("hello ${USER}") }}"#, "hello some-user");
    }
}
