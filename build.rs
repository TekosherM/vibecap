fn main() {
    println!("cargo:rerun-if-changed=assets/app_icon.ico");
    println!("cargo:rerun-if-changed=assets/app_icon.png");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/app_icon.ico");
    if let Err(e) = res.compile() {
        println!("cargo:warning=could not embed Windows exe icon: {e}");
    }
}
