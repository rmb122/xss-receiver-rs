use rand::{Rng, distr::Alphanumeric}; // 0.8

pub fn get_random_string(length: usize) -> String {
    return rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect();
}
