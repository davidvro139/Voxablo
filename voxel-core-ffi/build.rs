use std::path::PathBuf;

fn main() {
    let lib_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../project/native/bin/Release")
        .canonicalize()
        .expect("build project/native/voxel_wrapper.vcxproj (Release|x64) first");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rerun-if-changed={}", lib_dir.join("voxel_wrapper.lib").display());
}
