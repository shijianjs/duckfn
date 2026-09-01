use quack_rs::prelude::*;
use quack_rs::vector::vector_size;

pub fn register(reg: &impl Registrar) -> ExtResult<()> {
    let builders = vec![
        count_down()?,
        count_down_it()?,
    ];
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




fn count_down_it() -> Result<TableFunctionBuilder, ExtensionError> {
    TableFunctionBuilder::new("count_down_it")
        .named_param("start", TypeId::BigInt)
        // 1. bind closure: declare the output schema, read parameters,
        //    return the initial scan state.
        .with_state::<Counter, _>(|bind| {
            bind.add_result_column("n", TypeId::BigInt);
            let raw = unsafe { bind.get_named_parameter_value("start") };
            Ok(Counter {
                count: raw.as_i64_or(0).max(0) as usize,
            })
        })
        // 2. scan closure: mutate state, write rows, set chunk size.
        .scan(|state, chunk| {
            let size = vector_size();
            println!("size: {}", size);
            let mut writer = unsafe { chunk.writer(0) };
            for i in 0..size {
                let option = state.next();
                match option {
                    Some(value) => {
                        unsafe { writer.write_i64(i as usize, value as i64) } ;
                    },
                    None => {
                        unsafe { chunk.set_size(i as usize) };
                        return Ok(());
                    },
                }
            }
            unsafe { chunk.set_size(size as usize) };
            Ok(())
        })
        .build()
}

// First, the struct:

/// An iterator which counts from one to five
struct Counter {
    count: usize,
}

// we want our count to start at one, so let's add a new() method to help.
// This isn't strictly necessary, but is convenient. Note that we start
// `count` at zero, we'll see why in `next()`'s implementation below.
impl Counter {
    fn new(init: usize) -> Counter {
        Counter { count: init }
    }
}

// Then, we implement `Iterator` for our `Counter`:

impl Iterator for Counter {
    // we will be counting with usize
    type Item = usize;

    // next() is the only required method
    fn next(&mut self) -> Option<Self::Item> {
        // Increment our count. This is why we started at zero.

        // Check to see if we've finished counting or not.
        if self.count > 0 {
            self.count -= 1;
            Some(self.count)
        } else {
            None
        }
    }
}
