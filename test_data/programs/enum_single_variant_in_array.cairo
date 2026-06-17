#[derive(Drop)]
enum Wrapper {
    Only: felt252,
}

fn run_test(arr: Array<Wrapper>) -> felt252 {
    let mut arr = arr;
    let mut sum = 0;
    loop {
        match arr.pop_front() {
            Option::Some(w) => {
                match w {
                    Wrapper::Only(v) => { sum += v; },
                };
            },
            Option::None => { break; },
        };
    };
    sum
}
