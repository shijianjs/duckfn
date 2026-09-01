pub(crate) mod aggregate_function_adapter;
pub(crate) mod duck_args_type;
pub(crate) mod duck_register_builder;
pub(crate) mod scalar_function_adapter;
pub(crate) mod value_types;
pub(crate) mod helpers;

pub use aggregate_function_adapter::*;
pub use duck_args_type::*;
pub use duck_register_builder::*;
pub use scalar_function_adapter::*;
pub use value_types::*;
pub use helpers::*;


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
