//! File system helpers: workspace tree, language detection, file badges.

use gpui_kit::assets::IconName;
use gpui_kit::component::tree::TreeItem;
use std::path::Path;

/// Folders that are never worth walking for the explorer. They are still listed
/// (as folders, dimmed), just not opened.
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".next", "__pycache__", ".venv", "venv"];
/// Upper bound so a huge folder can't stall startup.
const MAX_ENTRIES: usize = 20_000;

/// Plain (Send) folder listing, built on a background thread.
pub struct Node {
    id: String,
    name: String,
    children: Option<Vec<Node>>,
}

/// Files the image tab shows (GPUI decodes these; SVG is rasterized sharp at any size).
pub fn is_image(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg" | "tif" | "tiff")
    )
}

/// Every leaf of a scan (files, and skipped folders listed without children).
pub fn flatten(nodes: &[Node], out: &mut Vec<String>) {
    for n in nodes {
        match &n.children {
            Some(ch) => flatten(ch, out),
            None => out.push(n.id.clone()),
        }
    }
}

pub fn scan(root: &Path) -> Vec<Node> {
    let mut budget = MAX_ENTRIES;
    walk(root, &mut budget)
}

/// Several folders, each as a top-level folder node (a multi-folder workspace).
pub fn scan_roots(roots: &[std::path::PathBuf]) -> Vec<Node> {
    let mut budget = MAX_ENTRIES;
    roots
        .iter()
        .map(|r| Node {
            id: r.to_string_lossy().into_owned(),
            name: r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| r.display().to_string()),
            children: Some(walk(r, &mut budget)),
        })
        .collect()
}

/// Turn a scan into tree items (UI thread: `TreeItem` is not Send), and collect
/// every folder's path: the tree only calls an item a folder when it has
/// children, so empty and skipped folders would otherwise draw as files.
pub fn to_items(nodes: Vec<Node>, dirs: &mut std::collections::HashSet<String>) -> Vec<TreeItem> {
    nodes
        .into_iter()
        .map(|n| match n.children {
            Some(ch) => {
                dirs.insert(n.id.clone());
                TreeItem::new(n.id, n.name).children(to_items(ch, dirs))
            }
            None => TreeItem::new(n.id, n.name),
        })
        .collect()
}

fn walk(dir: &Path, budget: &mut usize) -> Vec<Node> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<(bool, String, std::path::PathBuf)> = rd
        .flatten()
        .map(|e| {
            let p = e.path();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (is_dir, e.file_name().to_string_lossy().into_owned(), p)
        })
        .collect();
    // Folders first, then case-insensitive by name (like VS Code).
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));

    let mut out = Vec::with_capacity(entries.len());
    for (is_dir, name, path) in entries {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let id = path.to_string_lossy().into_owned();
        if is_dir {
            let children = if SKIP_DIRS.contains(&name.as_str()) { Vec::new() } else { walk(&path, budget) };
            out.push(Node { id, name, children: Some(children) });
        } else {
            out.push(Node { id, name, children: None });
        }
    }
    out
}

/// Languages the editor colors: (highlighter token, status-bar name, extensions,
/// file names). Tokens are names or extensions that syntect's grammar set
/// (bat's, about 200 grammars) resolves; the first entries keep the names
/// `complete.rs` uses for its keywords and snippets.
const LANGS: &[(&str, &str, &[&str], &[&str])] = &[
    ("javascript", "JavaScript", &["js", "mjs", "cjs", "jsx", "es6", "htc"], &[]),
    ("typescript", "TypeScript", &["ts", "mts", "cts"], &[]),
    ("tsx", "TypeScript JSX", &["tsx"], &[]),
    ("html", "HTML", &["html", "htm", "shtml", "xhtml", "astro", "hbs", "handlebars", "mustache", "njk"], &[]),
    ("css", "CSS", &["css", "pcss", "postcss"], &[]),
    (
        "json",
        "JSON",
        &["json", "jsonc", "json5", "webmanifest", "ipynb", "flx", "code-workspace", "har", "geojson", "jsonl", "ndjson"],
        &[".babelrc", ".eslintrc", ".prettierrc", ".swcrc", "composer.lock", "package-lock.json", ".jshintrc"],
    ),
    ("markdown", "Markdown", &["md", "markdown", "mdx", "mdown", "mkd"], &[]),
    (
        "python",
        "Python",
        &["py", "pyw", "pyi", "pyx", "pxd", "py3", "gyp", "bzl", "bazel"],
        &["sconstruct", "sconscript", "snakefile", "build.bazel"],
    ),
    ("rust", "Rust", &["rs"], &[]),
    ("php", "PHP", &["php", "phtml", "php3", "php4", "php5", "php7", "phpt"], &[]),
    ("toml", "TOML", &["toml", "tml"], &["cargo.lock", "pipfile", "poetry.lock", "uv.lock", "pdm.lock", "gopkg.lock"]),
    ("yaml", "YAML", &["yml", "yaml", "sublime-syntax"], &["yarn.lock", ".clang-format", ".clangd"]),
    (
        "bash",
        "Shell",
        &["sh", "bash", "zsh", "ash", "ksh", "ebuild", "eclass", "command"],
        &[".bashrc", ".bash_profile", ".bash_aliases", ".zshrc", ".zprofile", ".zshenv", ".profile", "pkgbuild", "apkbuild", "gradlew"],
    ),
    ("c", "C", &["c", "h"], &[]),
    ("cpp", "C++", &["cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++", "inl", "ipp", "tpp", "cu", "cuh", "ino"], &[]),
    ("go", "Go", &["go"], &[]),
    ("java", "Java", &["java", "bsh"], &[]),
    ("lua", "Lua", &["lua", "luau", "rockspec"], &[]),
    ("sql", "SQL", &["sql", "ddl", "dml", "psql", "pgsql", "mysql"], &[]),
    // The tokens below are syntect extensions.
    ("vue", "Vue", &["vue"], &[]),
    ("svelte", "Svelte", &["svelte"], &[]),
    ("erb", "Embedded template", &["ejs", "erb", "rhtml", "eex", "heex", "leex"], &[]),
    ("twig", "Twig", &["twig"], &[]),
    ("j2", "Jinja", &["j2", "jinja", "jinja2"], &[]),
    ("scss", "SCSS", &["scss"], &[]),
    ("sass", "Sass", &["sass"], &[]),
    ("less", "Less", &["less"], &[]),
    ("styl", "Stylus", &["styl", "stylus"], &[]),
    ("cs", "C#", &["cs", "csx", "cake"], &[]),
    ("fs", "F#", &["fs", "fsi", "fsx"], &[]),
    ("kt", "Kotlin", &["kt", "kts", "ktm"], &[]),
    ("groovy", "Groovy", &["groovy", "gvy", "gradle", "nf"], &["jenkinsfile"]),
    ("scala", "Scala", &["scala", "sbt", "sc", "mill"], &[]),
    ("swift", "Swift", &["swift"], &[]),
    ("dart", "Dart", &["dart"], &[]),
    ("m", "Objective-C", &["m"], &[]),
    ("mm", "Objective-C++", &["mm"], &[]),
    (
        "rb",
        "Ruby",
        &["rb", "rake", "gemspec", "ru", "rbw", "podspec", "jbuilder", "thor"],
        &["gemfile", "rakefile", "podfile", "vagrantfile", "brewfile", "fastfile", "appfile", "guardfile", "capfile", "berksfile"],
    ),
    ("pl", "Perl", &["pl", "pm", "pod", "t", "psgi"], &[]),
    ("hs", "Haskell", &["hs"], &[]),
    ("lhs", "Literate Haskell", &["lhs"], &[]),
    ("cabal", "Cabal", &["cabal"], &[]),
    ("ml", "OCaml", &["ml", "mli"], &[]),
    ("sml", "Standard ML", &["sml", "sig"], &[]),
    ("elm", "Elm", &["elm"], &[]),
    ("ex", "Elixir", &["ex", "exs"], &["mix.lock"]),
    ("erl", "Erlang", &["erl", "hrl", "escript"], &["rebar.config", "emakefile"]),
    ("clj", "Clojure", &["clj", "cljs", "cljc", "edn", "bb"], &[]),
    ("lisp", "Lisp", &["lisp", "lsp", "cl", "el", "scm", "ss", "sld", "fnl"], &[".emacs"]),
    ("rkt", "Racket", &["rkt", "rktl"], &[]),
    ("jl", "Julia", &["jl"], &[]),
    ("r", "R", &["r", "rprofile", "rmd"], &[".rprofile"]),
    ("nim", "Nim", &["nim", "nims", "nimble"], &[]),
    ("zig", "Zig", &["zig", "zon"], &[]),
    ("d", "D", &["d", "di"], &[]),
    ("pas", "Pascal", &["pas", "pp", "dpr", "lpr"], &[]),
    ("cr", "Crystal", &["cr"], &[]),
    ("purs", "PureScript", &["purs"], &[]),
    ("lean", "Lean", &["lean"], &[]),
    ("coffee", "CoffeeScript", &["coffee", "cson", "iced"], &["cakefile"]),
    ("ls", "LiveScript", &["ls"], &[]),
    ("tcl", "Tcl", &["tcl", "tk", "itcl"], &[]),
    ("f90", "Fortran", &["f90", "f95", "f03", "f08"], &[]),
    ("f", "Fortran (fixed form)", &["f", "f77", "for", "fpp"], &[]),
    ("asm", "Assembly", &["asm", "s", "nasm", "yasm", "a51", "inc"], &[]),
    ("ll", "LLVM IR", &["ll"], &[]),
    ("glsl", "GLSL", &["glsl", "vert", "frag", "geom", "tesc", "tese", "comp", "vsh", "fsh", "shader"], &[]),
    // There is no HLSL grammar in the set; GLSL's is the closest (C-like, same types).
    ("glsl", "HLSL", &["hlsl", "fx", "fxh", "cginc", "compute", "usf", "ush"], &[]),
    ("wgsl", "WGSL", &["wgsl"], &[]),
    ("sv", "SystemVerilog", &["sv", "svh"], &[]),
    ("v", "Verilog", &["v", "vh"], &[]),
    ("sol", "Solidity", &["sol"], &[]),
    ("vy", "Vyper", &["vy"], &[]),
    ("proto", "Protocol Buffers", &["proto", "protobuf"], &[]),
    ("textproto", "Protocol Buffers (text)", &["textproto", "pbtxt", "prototxt", "textpb"], &[]),
    ("graphql", "GraphQL", &["graphql", "graphqls", "gql"], &[]),
    ("tf", "Terraform", &["tf", "tfvars", "hcl", "nomad"], &[".terraform.lock.hcl"]),
    ("tfstate", "Terraform state", &["tfstate"], &[]),
    ("nix", "Nix", &["nix"], &[]),
    ("jsonnet", "Jsonnet", &["jsonnet", "libsonnet"], &[]),
    ("rego", "Rego", &["rego"], &[]),
    ("epp", "Puppet", &["epp"], &[]),
    ("qml", "QML", &["qml", "qmlproject"], &[]),
    ("robot", "Robot Framework", &["robot", "resource"], &[]),
    ("jq", "jq", &["jq"], &[]),
    ("Dockerfile", "Dockerfile", &["dockerfile", "containerfile"], &["dockerfile", "containerfile"]),
    ("Makefile", "Makefile", &["mk", "mak", "make"], &["makefile", "gnumakefile", "bsdmakefile"]),
    ("cmake", "CMake", &["cmake"], &["cmakelists.txt"]),
    ("ninja", "Ninja", &["ninja"], &[]),
    ("nsi", "NSIS", &["nsi", "nsh"], &[]),
    (
        "xml",
        "XML",
        &[
            "xml", "xsd", "xsl", "xslt", "xaml", "csproj", "fsproj", "vbproj", "vcxproj", "props", "targets", "resx", "nuspec", "plist",
            "storyboard", "xib", "wsdl", "rss", "atom", "opml", "kml", "gpx", "manifest", "xlf", "config", "pom", "fxml", "iml", "tld", "svg",
        ],
        &["pom.xml", "web.config", "app.config"],
    ),
    (
        "ini",
        "INI",
        &["ini", "cfg", "inf", "reg", "desktop", "url", "lng", "prefs"],
        &[".editorconfig", ".coveragerc", ".pylintrc", ".flake8", "setup.cfg", "tox.ini", ".npmrc", ".yarnrc", "pytest.ini", "mypy.ini", ".hgrc", ".gitlint"],
    ),
    ("properties", "Properties", &["properties"], &[]),
    ("nginx.conf", "Config", &["conf", "nginx"], &["nginx.conf", "mime.types"]),
    (".env", "Environment", &["env"], &[".env", ".envrc", ".flaskenv"]),
    (
        "gitignore",
        "Ignore list",
        &["gitignore", "dockerignore", "npmignore", "eslintignore", "prettierignore", "vscodeignore", "slugignore"],
        &[".gitignore", ".dockerignore", ".npmignore", ".eslintignore", ".prettierignore", ".vscodeignore", ".hgignore", ".gcloudignore", ".helmignore"],
    ),
    ("gitattributes", "Git attributes", &["gitattributes"], &[".gitattributes"]),
    ("gitconfig", "Git config", &["gitconfig"], &[".gitconfig", ".gitmodules"]),
    ("COMMIT_EDITMSG", "Git commit", &[], &["commit_editmsg", "merge_msg", "tag_editmsg"]),
    ("mailmap", "Git mailmap", &["mailmap"], &[".mailmap"]),
    ("csv", "CSV", &["csv", "tsv", "psv"], &[]),
    ("diff", "Diff", &["diff", "patch", "rej"], &[]),
    ("tex", "LaTeX", &["tex", "ltx", "sty", "cls", "dtx", "ins"], &[]),
    ("bib", "BibTeX", &["bib"], &[]),
    ("rst", "reStructuredText", &["rst"], &[]),
    ("adoc", "AsciiDoc", &["adoc", "asciidoc", "asc"], &[]),
    ("org", "Org", &["org"], &[]),
    ("textile", "Textile", &["textile"], &[]),
    ("wiki", "MediaWiki", &["wiki", "mediawiki"], &[]),
    ("man", "Manual page", &["man", "1", "2", "3", "4", "5", "6", "7", "8", "9", "groff", "troff"], &[]),
    ("log", "Log", &["log", "out"], &[]),
    ("http", "HTTP", &["http", "rest"], &[]),
    ("requirements.txt", "pip requirements", &[], &["requirements.txt", "requirements.in", "constraints.txt"]),
    ("bat", "Batch", &["bat", "cmd", "btm"], &[]),
    // There is no PowerShell grammar in the set; the shell one colors its strings, $variables and # comments.
    ("bash", "PowerShell", &["ps1", "psm1", "psd1"], &[]),
    ("fish", "Fish", &["fish"], &[]),
    ("awk", "AWK", &["awk", "gawk", "mawk", "nawk"], &[]),
    ("vim", "Vim script", &["vim", "vimrc"], &[".vimrc", "_vimrc", ".gvimrc"]),
    ("crontab", "Crontab", &["crontab"], &["crontab"]),
    ("ssh_config", "SSH config", &[], &["ssh_config"]),
    ("sshd_config", "SSHD config", &[], &["sshd_config"]),
    ("known_hosts", "Known hosts", &[], &["known_hosts", "authorized_keys"]),
    ("hosts", "Hosts", &[], &["hosts"]),
    ("gnuplot", "gnuplot", &["gp", "gpl", "gnuplot", "plt"], &[]),
    ("applescript", "AppleScript", &["applescript"], &[]),
    ("as", "ActionScript", &["as"], &[]),
    ("adb", "Ada", &["adb", "ads", "ada", "gpr"], &[]),
    ("cfm", "ColdFusion", &["cfm", "cfml", "cfc"], &[]),
    ("dot", "Graphviz", &["dot", "gv"], &[]),
    ("haml", "Haml", &["haml"], &[]),
    ("slim", "Slim", &["slim"], &[]),
    ("jsp", "JSP", &["jsp"], &[]),
    ("asp", "ASP", &["asp", "asa"], &[]),
    ("varlink", "Varlink", &["varlink"], &[]),
    ("eml", "Email", &["eml", "mbox"], &[]),
];

/// Highlighter token and status-bar name for a file: by file name, then
/// extension, then (for files with no known extension) the `#!` line.
pub fn language_for(path: &Path, first_line: &str) -> (&'static str, &'static str) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
    if let Some(l) = LANGS.iter().find(|l| l.3.contains(&name.as_str())) {
        return (l.0, l.1);
    }
    if name.starts_with(".env") {
        return (".env", "Environment");
    }
    if name.starts_with("dockerfile") || name.ends_with(".dockerfile") {
        return ("Dockerfile", "Dockerfile");
    }
    if name.starts_with("requirements") && name.ends_with(".txt") {
        return ("requirements.txt", "pip requirements");
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    if !ext.is_empty() {
        if let Some(l) = LANGS.iter().find(|l| l.2.contains(&ext)) {
            return (l.0, l.1);
        }
    }
    shebang(first_line).unwrap_or(("text", "Plain Text"))
}

/// `#!/usr/bin/env python3` and friends.
fn shebang(line: &str) -> Option<(&'static str, &'static str)> {
    let rest = line.strip_prefix("#!")?;
    let mut words = rest.split_whitespace();
    let mut prog = words.next()?.rsplit('/').next()?;
    if prog == "env" {
        prog = words.find(|w| !w.starts_with('-'))?;
    }
    let prog = prog.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    let token = match prog {
        "python" | "pypy" => "python",
        "node" | "nodejs" | "bun" | "deno" => "javascript",
        "sh" | "bash" | "zsh" | "dash" | "ksh" | "ash" => "bash",
        "fish" => "fish",
        "ruby" => "rb",
        "perl" => "pl",
        "php" => "php",
        "lua" => "lua",
        "tclsh" | "wish" => "tcl",
        "awk" | "gawk" => "awk",
        "Rscript" => "r",
        "julia" => "jl",
        "elixir" => "ex",
        "escript" => "erl",
        "make" => "Makefile",
        _ => return None,
    };
    LANGS.iter().find(|l| l.0 == token).map(|l| (l.0, l.1))
}


/// The explorer's, tabs' and search's file icon: one Lucide outline per kind of
/// file (code, data, text, image, config, script, lock, archive, ...), tinted
/// with the language's color, so every row lines up and reads the same way.
/// `None` means the neutral text color.
pub fn icon_for(name: &str) -> (IconName, Option<u32>) {
    let lower = name.to_ascii_lowercase();
    let ext = if lower.contains('.') { lower.rsplit('.').next().unwrap_or("") } else { "" };
    match (lower.as_str(), ext) {
        ("package-lock.json" | "cargo.lock" | "yarn.lock" | "pnpm-lock.yaml" | "composer.lock", _) | (_, "lock") => (IconName::FileLock, None),
        ("license" | "licence" | "copying", _) => (IconName::FileText, Some(0xd9a74a)),
        ("dockerfile" | "makefile" | ".gitignore" | ".gitattributes" | ".editorconfig" | ".npmrc" | ".env", _) => (IconName::FileCog, None),
        (_, "js" | "mjs" | "cjs") => (IconName::FileCode, Some(0xe8c547)),
        (_, "jsx") => (IconName::FileCode, Some(0x61dafb)),
        (_, "ts" | "mts" | "cts" | "tsx") => (IconName::FileCode, Some(0x3b8eea)),
        (_, "html" | "htm" | "vue" | "svelte") => (IconName::FileCode, Some(0xe8704a)),
        (_, "css" | "scss" | "less") => (IconName::FileCode, Some(0x5aa7e8)),
        (_, "py" | "pyw" | "pyi") => (IconName::FileCode, Some(0x4f9fd8)),
        (_, "rs") => (IconName::FileCode, Some(0xdea584)),
        (_, "php") => (IconName::FileCode, Some(0x8c8fd8)),
        (_, "c" | "h" | "cpp" | "hpp" | "cc" | "cs") => (IconName::FileCode, Some(0x6c9bd2)),
        (_, "go") => (IconName::FileCode, Some(0x4fd1c5)),
        (_, "java" | "kt") => (IconName::FileCode, Some(0xe76f51)),
        (_, "lua" | "rb" | "swift" | "dart" | "sql") => (IconName::FileCode, Some(0x5a7fe8)),
        (_, "json" | "jsonc" | "json5" | "webmanifest") => (IconName::FileBraces, Some(0xd9a74a)),
        (_, "md" | "markdown" | "mdx" | "txt" | "rst" | "log") => (IconName::FileText, Some(0x8fb8ff)),
        (_, "flx") => (IconName::FileText, Some(0xc792ea)),
        (_, "toml" | "yml" | "yaml" | "ini" | "cfg" | "conf" | "env" | "xml") => (IconName::FileCog, None),
        (_, "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd") => (IconName::FileTerminal, Some(0x5fd38d)),
        (_, "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "webp" | "bmp" | "avif") => (IconName::FileImage, Some(0xc792ea)),
        (_, "zip" | "7z" | "rar" | "gz" | "tgz" | "tar" | "whl" | "jar") => (IconName::FileArchive, None),
        (_, "mp3" | "wav" | "ogg" | "flac") => (IconName::FileMusic, None),
        (_, "mp4" | "mov" | "webm" | "mkv") => (IconName::FileVideoCamera, None),
        (_, "ttf" | "otf" | "woff" | "woff2") => (IconName::FileType, None),
        (_, "csv" | "tsv" | "xlsx") => (IconName::FileSpreadsheet, Some(0x5fd38d)),
        (_, "pem" | "key" | "crt") => (IconName::FileKey, None),
        (_, "pdf" | "docx" | "doc" | "odt" | "rtf" | "pptx" | "odp") => (IconName::FileText, Some(0xe8704a)),
        (_, "ods" | "xls") => (IconName::FileSpreadsheet, Some(0x5fd38d)),
        (_, "db" | "sqlite" | "sqlite3") => (IconName::FileBraces, Some(0x8fb8ff)),
        // Anything else the editor colors is code.
        _ => match language_for(Path::new(&lower), "").0 {
            "text" => (IconName::File, None),
            _ => (IconName::FileCode, None),
        },
    }
}

/// Folder icon: `.git` gets its own, everything else the plain folder.
pub fn folder_icon(name: &str, open: bool) -> IconName {
    match name {
        ".git" => IconName::FolderGit,
        _ if open => IconName::FolderOpen,
        _ => IconName::Folder,
    }
}

// The extra Lucide icons above, embedded next to gpui-kit's default set
// (about 0.5 KiB of SVG each); see `main.rs` for the asset source.
gpui_kit::assets::icon_assets!(pub FileIcons, [
    File, FileCode, FileBraces, FileText, FileImage, FileLock, FileCog, FileTerminal, FileArchive,
    FileMusic, FileVideoCamera, FileType, FileSpreadsheet, FileKey, FolderGit, Clock,
    // Source Control
    GitBranch, GitCommitHorizontal, GitPullRequest, CloudUpload, RefreshCcw, Rocket, FileDiff, Undo2,
    // Agent panel
    TextCursorInput, Brain, Pencil, AtSign, Paperclip, ShieldAlert
]);

#[cfg(test)]
mod tests {
    use super::language_for;
    use std::path::Path;

    #[test]
    fn languages() {
        let l = |p: &str, first: &str| language_for(Path::new(p), first).1;
        assert_eq!(l("a/app.py", ""), "Python");
        assert_eq!(l("config.yml", ""), "YAML");
        assert_eq!(l("Main.kt", ""), "Kotlin");
        assert_eq!(l("Dockerfile", ""), "Dockerfile");
        assert_eq!(l("Dockerfile.prod", ""), "Dockerfile");
        assert_eq!(l(".env.local", ""), "Environment");
        assert_eq!(l("requirements-dev.txt", ""), "pip requirements");
        assert_eq!(l("Gemfile", ""), "Ruby");
        assert_eq!(l("build.ps1", ""), "PowerShell");
        assert_eq!(l("run", "#!/usr/bin/env python3"), "Python");
        assert_eq!(l("deploy", "#!/bin/bash -e"), "Shell");
        assert_eq!(l("LOCK", ""), "Plain Text");
        assert_eq!(l("notes.txt", ""), "Plain Text");
    }
}
