#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("resources/buzzy-ro.ico");
    res.compile().unwrap();
}

#[cfg(unix)]
fn main() {}
