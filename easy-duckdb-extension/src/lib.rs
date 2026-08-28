pub mod aggregate_function_wrapper;
pub mod duck_args_type;
pub mod duck_register_builder;
pub mod scalar_function_wrapper;
pub mod value_types;
pub mod helpers;

pub use aggregate_function_wrapper::*;
pub use duck_args_type::*;
pub use duck_register_builder::*;
pub use scalar_function_wrapper::*;
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
