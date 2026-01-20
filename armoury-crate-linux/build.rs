use std::path::PathBuf;
use std::str::FromStr;

use slint_build::CompilerConfiguration;

fn main() {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut main = PathBuf::from_str(root).unwrap();
    main.push("ui/main_window.slint");

    let mut include = PathBuf::from_str(root).unwrap();
    include.push("ui");

    slint_build::print_rustc_flags().unwrap();
    slint_build::compile_with_config(
        main,
        CompilerConfiguration::new()
            .with_include_paths(vec![include])
            .with_style("fluent".into()),
    )
    .unwrap();
}
