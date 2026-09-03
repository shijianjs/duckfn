use easy_duckdb_extension::{DuckDataIterator, DuckOptionResult, DuckResult, TableFunctionAdapter};
use easy_duckdb_extension_macro::DuckStruct;
use quack_rs::prelude::*;
use quack_rs::vector::vector_size;
use std::iter::Map;
use std::ops::Range;

pub fn register(reg: &impl Registrar) -> ExtResult<()> {
    let builders = vec![
        count_down()?,
        count_down_it()?,
        CountDownS::table_function_builder()?,
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
                        unsafe { writer.write_i64(i as usize, value as i64) };
                    }
                    None => {
                        unsafe { chunk.set_size(i as usize) };
                        return Ok(());
                    }
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

#[derive(Default, Debug, Clone, DuckStruct)]
#[duck(named_param_from = "start")]
struct CountDownS {
    start: i64,
}

#[derive(Default, Debug, Clone, DuckStruct)]
struct CountDownOutput {
    n: i64,
}

impl TableFunctionAdapter for CountDownS {
    const NAME: &'static str = "count_down_s";
    type Args = Self;
    type Output = CountDownOutput;

    fn init_data_iterator(
        args: Self,
    ) -> DuckResult<DuckDataIterator<Self::Output>> {
        // .collect::<Vec<_>>();
        Ok(Box::new((0..args.start).rev()
            .map(|x| {
                if x==5{
                    Ok(None)
                }else {
                    Ok(Some(CountDownOutput { n: x }))
                }
            })))
    }
}
// 全功能
//底层/高级模式
fn demo1() -> DuckResult<DuckDataIterator<CountDownOutput>> {
    Ok(Box::new(demo4().map(|x| Ok(Some(x)))))
}
// 这档没太大必要，仅仅是消了个外层的Ok而已，参数异常又是常见异常
fn demo2() -> DuckDataIterator<CountDownOutput> {
    todo!()
}
// 可以处理入参异常，出去的不处理
//创建数据源可能失败，但 iterator 一旦创建成功，后续只产生正常数据。
fn demo3() -> DuckResult<impl Iterator<Item = CountDownOutput>> {
    Ok(demo4())
}
// 都不处理
//参数已经成功解析，后面不会产生 error，也不会产生 NULL。
fn demo4() -> impl Iterator<Item = CountDownOutput> {
    (0..10).rev()
        .map(|x| CountDownOutput { n: x })
}
