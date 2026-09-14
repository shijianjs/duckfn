pub(crate) mod duck_columns;
pub(crate) mod value_types;
pub(crate) mod register;
pub(crate) mod functions;
pub(crate) mod utils;

pub use duckfn_macro::*;
pub use duck_columns::*;
pub use functions::*;
pub use value_types::*;
pub use utils::*;
pub use register::*;
pub use inventory::submit as inventory_submit;




pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
