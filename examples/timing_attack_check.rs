use dudect_bencher::{BenchRng, Class, CtRunner, ctbench_main};

use dudect_bencher::rand::RngExt;
use rand::distr::{Alphanumeric, SampleString};
use relay::domain::NonEmptyString;
use relay::verify_api_key;

fn bench_verify_api_key(runner: &mut CtRunner, rng: &mut BenchRng) {
    const SET_SIZE: usize = 100_000;

    let expected = NonEmptyString::new(String::from("super_secret_api_key_12345")).unwrap();

    let mut classes = Vec::with_capacity(SET_SIZE);
    let mut inputs = Vec::with_capacity(SET_SIZE);

    for _ in 0..SET_SIZE {
        if rng.random::<bool>() {
            classes.push(Class::Right);
            inputs.push(String::from("super_secret_api_key_12345"));
        } else {
            classes.push(Class::Left);
            let random_string = Alphanumeric.sample_string(rng, expected.len());
            inputs.push(random_string);
        }
    }

    for (class, input) in classes.into_iter().zip(inputs) {
        runner.run_one(class, || verify_api_key(Some(&expected), Some(&input)));
    }
}

ctbench_main!(bench_verify_api_key);
