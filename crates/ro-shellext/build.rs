fn main() {
    println!("cargo:rerun-if-changed=../../assets/app.rc");
    println!("cargo:rerun-if-changed=../../assets/app.ico");
    embed_resource::compile_for_everything("../../assets/app.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("compile app.rc");
}
