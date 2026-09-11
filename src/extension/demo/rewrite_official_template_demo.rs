use duckfn::{duck_scalar_function, duck_table_function, DuckStruct};

#[duck_scalar_function]
fn rusty_echo(s: String) -> String {
    format!("🐤 {s} 🦀 {s}")
}

#[derive(Clone, Debug, DuckStruct)]
pub struct RustyQuackResult {
    column0: String,
}

#[duck_table_function]
fn rusty_quack(name: String) -> impl Iterator<Item = RustyQuackResult> {
    vec![RustyQuackResult {
        column0: format!("Rusty Quack {} 🐥", name),
    }]
    .into_iter()
}
