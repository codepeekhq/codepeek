use std::path::Path;

/// Detect tree-sitter language name from file extension.
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
        "zig" => Some("zig"),
        "swift" => Some("swift"),
        "kt" | "kts" => Some("kotlin"),
        "scala" => Some("scala"),
        "ex" | "exs" => Some("elixir"),
        "erl" | "hrl" => Some("erlang"),
        "hs" => Some("haskell"),
        "ml" | "mli" => Some("ocaml"),
        "r" | "R" => Some("r"),
        "sql" => Some("sql"),
        "tf" | "hcl" => Some("hcl"),
        "Dockerfile" => Some("dockerfile"),
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
}
