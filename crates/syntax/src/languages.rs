use std::path::Path;

pub fn detect_language(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "rs" => Some("rust"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "jsx" => Some("jsx"),
        "py" | "pyi" => Some("python"),
        "go" => Some("go"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp"),
        "java" => Some("java"),
        "rb" => Some("ruby"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "json" => Some("json"),
        "md" | "markdown" => Some("markdown"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        "sh" | "bash" => Some("bash"),
        "lua" => Some("lua"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rust_extension() {
        assert_eq!(detect_language(Path::new("main.rs")), Some("rust"));
    }

    #[test]
    fn javascript_extensions() {
        assert_eq!(detect_language(Path::new("app.js")), Some("javascript"));
        assert_eq!(detect_language(Path::new("lib.mjs")), Some("javascript"));
        assert_eq!(detect_language(Path::new("util.cjs")), Some("javascript"));
    }

    #[test]
    fn typescript_extensions() {
        assert_eq!(detect_language(Path::new("app.ts")), Some("typescript"));
        assert_eq!(detect_language(Path::new("app.tsx")), Some("tsx"));
    }

    #[test]
    fn python_extension() {
        assert_eq!(detect_language(Path::new("script.py")), Some("python"));
        assert_eq!(detect_language(Path::new("types.pyi")), Some("python"));
    }

    #[test]
    fn cpp_extensions() {
        assert_eq!(detect_language(Path::new("main.cpp")), Some("cpp"));
        assert_eq!(detect_language(Path::new("main.cc")), Some("cpp"));
        assert_eq!(detect_language(Path::new("header.hpp")), Some("cpp"));
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert_eq!(detect_language(Path::new("file.xyz")), None);
    }

    #[test]
    fn no_extension_returns_none() {
        assert_eq!(detect_language(Path::new("Makefile")), None);
    }

    #[test]
    fn path_with_directories() {
        let path = PathBuf::from("src/lib/parser.rs");
        assert_eq!(detect_language(&path), Some("rust"));
    }

    #[test]
    fn shell_extensions() {
        assert_eq!(detect_language(Path::new("script.sh")), Some("bash"));
        assert_eq!(detect_language(Path::new("setup.bash")), Some("bash"));
    }

    #[test]
    fn config_file_extensions() {
        assert_eq!(detect_language(Path::new("config.toml")), Some("toml"));
        assert_eq!(detect_language(Path::new("config.yaml")), Some("yaml"));
        assert_eq!(detect_language(Path::new("config.yml")), Some("yaml"));
        assert_eq!(detect_language(Path::new("data.json")), Some("json"));
    }

    #[test]
    fn jsx_extension() {
        assert_eq!(detect_language(Path::new("component.jsx")), Some("jsx"));
    }

    #[test]
    fn go_extension() {
        assert_eq!(detect_language(Path::new("main.go")), Some("go"));
    }

    #[test]
    fn c_extensions() {
        assert_eq!(detect_language(Path::new("main.c")), Some("c"));
        assert_eq!(detect_language(Path::new("header.h")), Some("c"));
    }

    #[test]
    fn java_extension() {
        assert_eq!(detect_language(Path::new("Main.java")), Some("java"));
    }

    #[test]
    fn ruby_extension() {
        assert_eq!(detect_language(Path::new("app.rb")), Some("ruby"));
    }

    #[test]
    fn markup_extensions() {
        assert_eq!(detect_language(Path::new("page.html")), Some("html"));
        assert_eq!(detect_language(Path::new("page.htm")), Some("html"));
        assert_eq!(detect_language(Path::new("style.css")), Some("css"));
        assert_eq!(detect_language(Path::new("readme.md")), Some("markdown"));
        assert_eq!(
            detect_language(Path::new("readme.markdown")),
            Some("markdown")
        );
    }

    #[test]
    fn lua_extension() {
        assert_eq!(detect_language(Path::new("init.lua")), Some("lua"));
    }

    #[test]
    fn unsupported_languages_return_none() {
        assert_eq!(detect_language(Path::new("main.zig")), None);
        assert_eq!(detect_language(Path::new("main.swift")), None);
        assert_eq!(detect_language(Path::new("main.kt")), None);
        assert_eq!(detect_language(Path::new("main.scala")), None);
        assert_eq!(detect_language(Path::new("main.ex")), None);
    }

    #[test]
    fn mts_and_cts_extensions() {
        assert_eq!(detect_language(Path::new("module.mts")), Some("typescript"));
        assert_eq!(detect_language(Path::new("module.cts")), Some("typescript"));
    }

    #[test]
    fn tsx_extension() {
        assert_eq!(detect_language(Path::new("component.tsx")), Some("tsx"));
    }

    #[test]
    fn pyi_extension() {
        assert_eq!(detect_language(Path::new("stubs.pyi")), Some("python"));
    }

    #[test]
    fn cpp_cxx_and_hxx_extensions() {
        assert_eq!(detect_language(Path::new("main.cxx")), Some("cpp"));
        assert_eq!(detect_language(Path::new("header.hxx")), Some("cpp"));
    }
}
