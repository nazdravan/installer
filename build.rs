// Pune în executabilul de Windows descrierea și manifestul care cere drepturi de administrator.
fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    if std::path::Path::new("assets/instalator.ico").exists() {
        res.set_icon("assets/instalator.ico");
    }
    res.set("FileDescription", "Instalator");
    res.set("ProductName", "Instalator");
    res.set_manifest_file("app.manifest");

    // La compilarea din macOS/Linux pentru ținta win7, winresource nu ghicește prefixul uneltelor MinGW.
    let target = std::env::var("TARGET").unwrap_or_default();
    if cfg!(not(windows)) && target.ends_with("windows-gnu") {
        let prefix = if target.starts_with("i686") { "i686-w64-mingw32" } else { "x86_64-w64-mingw32" };
        res.set_windres_path(&format!("{prefix}-windres"));
        res.set_ar_path(&format!("{prefix}-ar"));
    }
    if let Err(e) = res.compile() {
        // Fără manifest Windows n-ar cere drepturi de administrator: build-ul de release trebuie să eșueze.
        if std::env::var("PROFILE").as_deref() == Ok("release") {
            panic!("nu am putut adăuga manifestul: {e}");
        }
        println!("cargo:warning=nu am putut adăuga manifestul: {e}");
    }
}
