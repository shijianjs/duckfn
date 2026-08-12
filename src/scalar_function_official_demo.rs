use duckdb::core::{DataChunkHandle, Inserter, LogicalTypeId};
use duckdb::types::DuckString;
use duckdb::vscalar::{ScalarFunctionSignature, VScalar};
use duckdb::vtab::arrow::WritableVector;
use libduckdb_sys::duckdb_string_t;

struct EchoScalar;

impl VScalar for EchoScalar {
    type State = ();

    fn invoke(
        _state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input_vec = input.flat_vector(0);
        let values = unsafe { input_vec.as_slice_with_len::<duckdb_string_t>(input.len()) };
        let mut output = output.flat_vector();

        for (i, value) in values.iter().enumerate() {
            if input_vec.row_is_null(i as u64) {
                output.set_null(i);
                continue;
            }

            let mut value = *value;
            let s = DuckString::new(&mut value).as_str();
            output.insert(i, format!("🐤 {s} 🦀 {s}").as_str());
        }
        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![LogicalTypeId::Varchar.into()],
            LogicalTypeId::Varchar.into(),
        )]
    }
}