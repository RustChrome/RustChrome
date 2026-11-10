//! Embeds the application icon and version resource into the executable.

fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/rustchrome.ico");

    #[cfg(windows)]
    {
        // Links the compiled .res into the binary. embed-resource reports
        // NotWindows/Ok/failure as a value; a missing rc.exe only skips the
        // resource rather than failing the build.
        let _ = embed_resource::compile("assets/app.rc", embed_resource::NONE);
    }
}
