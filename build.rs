fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();

        res.set_icon("icon.ico");
        res.compile().expect("Failed to embed Windows icon");
    }
}
