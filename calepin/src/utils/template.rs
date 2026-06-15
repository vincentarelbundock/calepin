use minijinja::{AutoEscape, Environment};

pub fn no_autoescape_env<'source>() -> Environment<'source> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env
}
