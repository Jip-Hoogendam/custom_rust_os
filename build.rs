fn main() {
    cc::Build::new()
        .file("src/multi_tasking/premtive/context.S")
        .compile("context_restore");
}
