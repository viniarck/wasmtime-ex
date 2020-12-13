mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[wasm_bindgen]
pub fn plus_10(a: i32) -> i32 {
    a + 10
}

#[wasm_bindgen]
pub fn min(a: i32, b: i32) -> i32 {
    if a <= b {
        a
    } else {
        b
    }
}

#[wasm_bindgen]
pub fn sum(from: i32, to: i32) -> i32 {
    let mut acc = 0;
    for i in from..=to {
       acc += i
    }
    acc
}
