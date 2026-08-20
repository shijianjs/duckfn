use quack_rs::prelude::*;

pub fn register(reg: &impl Registrar) -> ExtResult<()> {
    let builders = vec![count_down()?];
    for builder in builders {
        unsafe { reg.register_table(builder) }?;
    }
    Ok(())
}

struct State {
    remaining: u64,
}

fn count_down() -> Result<TableFunctionBuilder, ExtensionError> {
    TableFunctionBuilder::new("count_down")
        .named_param("start", TypeId::BigInt)
        // 1. bind closure: declare the output schema, read parameters,
        //    return the initial scan state.
        .with_state::<State, _>(|bind| {
            bind.add_result_column("n", TypeId::BigInt);
            let raw = unsafe { bind.get_named_parameter_value("start") };
            Ok(State {
                remaining: raw.as_i64_or(0).max(0) as u64,
            })
        })
        // 2. scan closure: mutate state, write rows, set chunk size.
        .scan(|state, chunk| {
            if state.remaining == 0 {
                unsafe { chunk.set_size(0) };
                return Ok(());
            }
            
            let mut writer = unsafe { chunk.writer(0) };
            unsafe { writer.write_i64(0, state.remaining as i64) };
            state.remaining -= 1;
            unsafe { chunk.set_size(1) };
            Ok(())
        })
        .build()
}
