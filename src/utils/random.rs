use rand::{
    Rng,
    distr::{Alphanumeric, StandardUniform},
};

pub fn get_random_string(length: usize) -> String {
    return rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect();
}

pub fn get_random_bytes(length: usize) -> Vec<u8> {
    return rand::rng()
        .sample_iter(&StandardUniform)
        .take(length)
        .collect();
}
