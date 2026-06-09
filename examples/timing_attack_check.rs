use dudect_bencher::{BenchRng, Class, CtRunner, ctbench_main};

use dudect_bencher::rand::RngExt;
use subtle::ConstantTimeEq;

#[inline(never)]
fn verify_api_key(expected: Option<&String>, provided: Option<&str>) -> bool {
    expected.zip(provided).is_some_and(|(key, header)| {
        let key_bytes = key.as_bytes();
        let header_bytes = header.as_bytes();
        key_bytes.ct_eq(&header_bytes).into()
    })
}

fn bench_verify_api_key(runner: &mut CtRunner, rng: &mut BenchRng) {
    const SET_SIZE: usize = 100_000;

    let expected = String::from("super_secret_api_key_12345");
    let correct_input = "super_secret_api_key_12345";
    let incorrect_input = "Zuper_secret_api_key_12345";

    let mut classes = Vec::with_capacity(SET_SIZE);
    let mut inputs = Vec::with_capacity(SET_SIZE);

    for _ in 0..SET_SIZE {
        if rng.random::<bool>() {
            classes.push(Class::Right);
            inputs.push(correct_input);
        } else {
            classes.push(Class::Left);
            inputs.push(incorrect_input);
        }
    }

    for (class, input) in classes.into_iter().zip(inputs.into_iter()) {
        runner.run_one(class, || {
            verify_api_key(Some(&expected), Some(input));
        });
    }
}

ctbench_main!(bench_verify_api_key);
