mod args;
mod converter;

use args::PatchArgs;
use converter::uop_to_mul;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn convert_uop_to_mul(src: &str, output: &str, file: &str) {
    uop_to_mul(&PatchArgs {
        source_dir: src.to_owned(),
        output_dir: output.to_owned(),
        file_to_process: file.to_owned(),
    })
    .unwrap();
}
