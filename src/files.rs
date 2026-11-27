use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    "__pycache__",
    ".venv",
    "venv",
    ".cache",
    ".local",
    ".cargo",
    ".rustup",
    ".npm",
    ".nvm",
    ".bun",
    ".pnpm-store",
    ".yarn",
    ".deno",
    ".volta",
    ".asdf",
    ".android",
    ".gradle",
    ".m2",
    ".vscode-server",
    ".ollama",
    ".oh-my-zsh",
    ".dotnet",
    ".nuget",
    ".conda",
    ".pyenv",
    ".sdkman",
    ".gem",
    ".pub-cache",
    ".stack",
    ".ghcup",
    ".cabal",
    ".terraform.d",
    ".svelte-kit",
    ".nuxt",
    ".output",
    ".turbo",
    ".parcel-cache",
    ".vite",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    "coverage",
    "__snapshots__",
    "Pods",
    "DerivedData",
    ".zig-cache",
    "zig-out",
];
pub(crate) const MAX_ENTRIES: usize = 150_000;

fn cmp_name(a: &str, b: &str) -> std::cmp::Ordering {
    if a.is_ascii() && b.is_ascii() {
        for (x, y) in a.bytes().zip(b.bytes()) {
            let ord = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        return a.len().cmp(&b.len());
    }
    a.to_lowercase().cmp(&b.to_lowercase())
}

#[derive(Clone)]
pub struct Node {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) children: Option<Vec<Node>>,
    pub(crate) loaded: bool,
}

pub fn is_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg" | "tif" | "tiff")
    )
}

pub fn scan_paths(root: &Path, max: usize) -> Vec<String> {
    if let Some(mut git) = crate::core::git_files(root) {
        git.truncate(max);
        return git
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
    }
    let mut out = Vec::new();
    let mut queue = std::collections::VecDeque::from([root.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        if out.len() >= max {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            if out.len() >= max {
                break;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                let name = e.file_name();
                if let Some(s) = name.to_str() {
                    if SKIP_DIRS.iter().any(|skip| s.eq_ignore_ascii_case(skip)) {
                        continue;
                    }
                }
                queue.push_back(e.path());
            } else if ft.is_file() {
                out.push(e.path().to_string_lossy().into_owned());
            }
        }
    }
    out
}

pub fn nodes_from_remote(dir: &Path, entries: Vec<(String, bool)>) -> Vec<Node> {
    let mut entries = entries;
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| cmp_name(&a.0, &b.0)));
    entries
        .into_iter()
        .map(|(name, is_dir)| {
            let id = dir.join(&name).to_string_lossy().into_owned();
            let id = if dir.as_os_str().to_string_lossy().starts_with(r"\\") {
                id.replace('/', r"\")
            } else {
                id
            };
            if is_dir {
                Node {
                    id,
                    name,
                    children: Some(Vec::new()),
                    loaded: false,
                }
            } else {
                Node {
                    id,
                    name,
                    children: None,
                    loaded: true,
                }
            }
        })
        .collect()
}

pub fn scan_shallow(root: &Path) -> Vec<Node> {
    list(root)
}

pub fn scan_roots_shallow(roots: &[std::path::PathBuf]) -> Vec<Node> {
    roots
        .iter()
        .map(|r| Node {
            id: r.to_string_lossy().into_owned(),
            name: r
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| r.display().to_string()),
            children: Some(list(r)),
            loaded: true,
        })
        .collect()
}

pub fn scan_dir(dir: &Path) -> Vec<Node> {
    list(dir)
}

fn list(dir: &Path) -> Vec<Node> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<(bool, String, std::path::PathBuf)> = rd
        .flatten()
        .map(|e| {
            let p = e.path();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (is_dir, e.file_name().to_string_lossy().into_owned(), p)
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| cmp_name(&a.1, &b.1)));
    entries
        .into_iter()
        .map(|(is_dir, name, path)| {
            let id = path.to_string_lossy().into_owned();
            if is_dir {
                Node {
                    id,
                    name,
                    children: Some(Vec::new()),
                    loaded: false,
                }
            } else {
                Node {
                    id,
                    name,
                    children: None,
                    loaded: true,
                }
            }
        })
        .collect()
}

pub fn merge_nodes(old: Vec<Node>, new: Vec<Node>) -> Vec<Node> {
    let mut old_map: std::collections::HashMap<String, Node> =
        old.into_iter().map(|n| (n.id.clone(), n)).collect();
    new.into_iter()
        .map(|mut n| {
            if let Some(o) = old_map.remove(&n.id) {
                if n.children.is_some() {
                    if !n.loaded && o.loaded {
                        n.children = o.children;
                        n.loaded = true;
                    } else if n.loaded && o.loaded {
                        if let (Some(old_ch), Some(new_ch)) = (o.children, n.children.take()) {
                            n.children = Some(merge_nodes(old_ch, new_ch));
                        }
                    }
                }
            }
            n
        })
        .collect()
}

pub fn hydrate_expanded(
    nodes: &mut [Node],
    expanded: &HashSet<String>,
    list_dir: &dyn Fn(&Path) -> Vec<Node>,
) {
    for n in nodes.iter_mut() {
        if n.children.is_some() && expanded.contains(&n.id) {
            let children = list_dir(Path::new(&n.id));
            n.children = Some(children);
            n.loaded = true;
            if let Some(ch) = n.children.as_mut() {
                hydrate_expanded(ch, expanded, list_dir);
            }
        } else if n.loaded {
            if let Some(ch) = n.children.as_mut() {
                hydrate_expanded(ch, expanded, list_dir);
            }
        }
    }
}

pub fn find_node<'a>(nodes: &'a mut [Node], id: &str) -> Option<&'a mut Node> {
    for n in nodes {
        if n.id == id {
            return Some(n);
        }
        if let Some(ch) = &mut n.children {
            if let Some(found) = find_node(ch, id) {
                return Some(found);
            }
        }
    }
    None
}

pub fn contains_node(nodes: &[Node], id: &str) -> bool {
    for n in nodes {
        if n.id == id {
            return true;
        }
        if let Some(ch) = &n.children {
            if contains_node(ch, id) {
                return true;
            }
        }
    }
    false
}

pub fn ancestor_dirs(path: &Path, roots: &[PathBuf]) -> Vec<String> {
    let Some(root) = roots.iter().find(|r| path.starts_with(r)) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    if roots.len() > 1 {
        dirs.push(root.to_string_lossy().into_owned());
    }
    let mut cur = root.clone();
    if let Ok(rel) = path.strip_prefix(root) {
        let comps: Vec<_> = rel.components().collect();
        for comp in comps.iter().take(comps.len().saturating_sub(1)) {
            cur.push(comp);
            dirs.push(cur.to_string_lossy().into_owned());
        }
    }
    dirs
}

#[cfg(test)]
fn walk(dir: &Path, budget: &mut usize) -> Vec<Node> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<(bool, String, std::path::PathBuf)> = rd
        .flatten()
        .map(|e| {
            let p = e.path();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (is_dir, e.file_name().to_string_lossy().into_owned(), p)
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| cmp_name(&a.1, &b.1)));

    let mut out = Vec::with_capacity(entries.len());
    let mut dirs: Vec<(usize, String, std::path::PathBuf)> = Vec::new();
    for (is_dir, name, path) in entries {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let id = path.to_string_lossy().into_owned();
        if is_dir {
            dirs.push((out.len(), name.clone(), path));
            out.push(Node {
                id,
                name,
                children: Some(Vec::new()),
                loaded: true,
            });
        } else {
            out.push(Node {
                id,
                name,
                children: None,
                loaded: true,
            });
        }
    }
    for (ix, name, path) in dirs {
        if *budget == 0 {
            break;
        }
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        out[ix].children = Some(walk(&path, budget));
    }
    out
}

const LANGS: &[(&str, &str, &[&str], &[&str])] = &[
    (
        "javascript",
        "JavaScript",
        &["js", "mjs", "cjs", "jsx", "es6", "htc"],
        &[],
    ),
    ("typescript", "TypeScript", &["ts", "mts", "cts"], &[]),
    ("tsx", "TypeScript JSX", &["tsx"], &[]),
    (
        "html",
        "HTML",
        &[
            "html",
            "htm",
            "shtml",
            "xhtml",
            "astro",
            "hbs",
            "handlebars",
            "mustache",
            "njk",
        ],
        &[],
    ),
    ("css", "CSS", &["css", "pcss", "postcss"], &[]),
    (
        "json",
        "JSON",
        &[
            "json",
            "jsonc",
            "json5",
            "webmanifest",
            "ipynb",
            "flx",
            "code-workspace",
            "har",
            "geojson",
            "jsonl",
            "ndjson",
        ],
        &[
            ".babelrc",
            ".eslintrc",
            ".prettierrc",
            ".swcrc",
            "composer.lock",
            "package-lock.json",
            ".jshintrc",
        ],
    ),
    (
        "markdown",
        "Markdown",
        &["md", "markdown", "mdx", "mdown", "mkd"],
        &[],
    ),
    ("markdown", "Flux", &["flux"], &[]),
    (
        "python",
        "Python",
        &[
            "py", "pyw", "pyi", "pyx", "pxd", "py3", "gyp", "bzl", "bazel",
        ],
        &["sconstruct", "sconscript", "snakefile", "build.bazel"],
    ),
    ("rust", "Rust", &["rs"], &[]),
    (
        "php",
        "PHP",
        &["php", "phtml", "php3", "php4", "php5", "php7", "phpt"],
        &[],
    ),
    (
        "toml",
        "TOML",
        &["toml", "tml"],
        &[
            "cargo.lock",
            "pipfile",
            "poetry.lock",
            "uv.lock",
            "pdm.lock",
            "gopkg.lock",
        ],
    ),
    (
        "yaml",
        "YAML",
        &["yml", "yaml", "sublime-syntax"],
        &["yarn.lock", ".clang-format", ".clangd"],
    ),
    (
        "bash",
        "Shell",
        &[
            "sh", "bash", "zsh", "ash", "ksh", "ebuild", "eclass", "command",
        ],
        &[
            ".bashrc",
            ".bash_profile",
            ".bash_aliases",
            ".zshrc",
            ".zprofile",
            ".zshenv",
            ".profile",
            "pkgbuild",
            "apkbuild",
            "gradlew",
        ],
    ),
    ("c", "C", &["c", "h"], &[]),
    (
        "cpp",
        "C++",
        &[
            "cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++", "inl", "ipp", "tpp", "cu", "cuh",
            "ino",
        ],
        &[],
    ),
    ("go", "Go", &["go"], &[]),
    ("java", "Java", &["java", "bsh"], &[]),
    ("lua", "Lua", &["lua", "luau", "rockspec"], &[]),
    (
        "sql",
        "SQL",
        &["sql", "ddl", "dml", "psql", "pgsql", "mysql"],
        &[],
    ),
    ("vue", "Vue", &["vue"], &[]),
    ("svelte", "Svelte", &["svelte"], &[]),
    (
        "erb",
        "Embedded template",
        &["ejs", "erb", "rhtml", "eex", "heex", "leex"],
        &[],
    ),
    ("twig", "Twig", &["twig"], &[]),
    ("j2", "Jinja", &["j2", "jinja", "jinja2"], &[]),
    ("scss", "SCSS", &["scss"], &[]),
    ("sass", "Sass", &["sass"], &[]),
    ("less", "Less", &["less"], &[]),
    ("styl", "Stylus", &["styl", "stylus"], &[]),
    ("cs", "C#", &["cs", "csx", "cake"], &[]),
    ("fs", "F#", &["fs", "fsi", "fsx"], &[]),
    ("kt", "Kotlin", &["kt", "kts", "ktm"], &[]),
    (
        "groovy",
        "Groovy",
        &["groovy", "gvy", "gradle", "nf"],
        &["jenkinsfile"],
    ),
    ("scala", "Scala", &["scala", "sbt", "sc", "mill"], &[]),
    ("swift", "Swift", &["swift"], &[]),
    ("dart", "Dart", &["dart"], &[]),
    ("m", "Objective-C", &["m"], &[]),
    ("mm", "Objective-C++", &["mm"], &[]),
    (
        "rb",
        "Ruby",
        &[
            "rb", "rake", "gemspec", "ru", "rbw", "podspec", "jbuilder", "thor",
        ],
        &[
            "gemfile",
            "rakefile",
            "podfile",
            "vagrantfile",
            "brewfile",
            "fastfile",
            "appfile",
            "guardfile",
            "capfile",
            "berksfile",
        ],
    ),
    ("pl", "Perl", &["pl", "pm", "pod", "t", "psgi"], &[]),
    ("hs", "Haskell", &["hs"], &[]),
    ("lhs", "Literate Haskell", &["lhs"], &[]),
    ("cabal", "Cabal", &["cabal"], &[]),
    ("ml", "OCaml", &["ml", "mli"], &[]),
    ("sml", "Standard ML", &["sml", "sig"], &[]),
    ("elm", "Elm", &["elm"], &[]),
    ("ex", "Elixir", &["ex", "exs"], &["mix.lock"]),
    (
        "erl",
        "Erlang",
        &["erl", "hrl", "escript"],
        &["rebar.config", "emakefile"],
    ),
    ("clj", "Clojure", &["clj", "cljs", "cljc", "edn", "bb"], &[]),
    (
        "lisp",
        "Lisp",
        &["lisp", "lsp", "cl", "el", "scm", "ss", "sld", "fnl"],
        &[".emacs"],
    ),
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
    (
        "coffee",
        "CoffeeScript",
        &["coffee", "cson", "iced"],
        &["cakefile"],
    ),
    ("ls", "LiveScript", &["ls"], &[]),
    ("tcl", "Tcl", &["tcl", "tk", "itcl"], &[]),
    ("f90", "Fortran", &["f90", "f95", "f03", "f08"], &[]),
    (
        "f",
        "Fortran (fixed form)",
        &["f", "f77", "for", "fpp"],
        &[],
    ),
    (
        "asm",
        "Assembly",
        &["asm", "s", "nasm", "yasm", "a51", "inc"],
        &[],
    ),
    ("ll", "LLVM IR", &["ll"], &[]),
    (
        "glsl",
        "GLSL",
        &[
            "glsl", "vert", "frag", "geom", "tesc", "tese", "comp", "vsh", "fsh", "shader",
        ],
        &[],
    ),
    (
        "glsl",
        "HLSL",
        &["hlsl", "fx", "fxh", "cginc", "compute", "usf", "ush"],
        &[],
    ),
    ("wgsl", "WGSL", &["wgsl"], &[]),
    ("sv", "SystemVerilog", &["sv", "svh"], &[]),
    ("v", "Verilog", &["v", "vh"], &[]),
    ("sol", "Solidity", &["sol"], &[]),
    ("vy", "Vyper", &["vy"], &[]),
    ("proto", "Protocol Buffers", &["proto", "protobuf"], &[]),
    (
        "textproto",
        "Protocol Buffers (text)",
        &["textproto", "pbtxt", "prototxt", "textpb"],
        &[],
    ),
    ("graphql", "GraphQL", &["graphql", "graphqls", "gql"], &[]),
    (
        "tf",
        "Terraform",
        &["tf", "tfvars", "hcl", "nomad"],
        &[".terraform.lock.hcl"],
    ),
    ("tfstate", "Terraform state", &["tfstate"], &[]),
    ("nix", "Nix", &["nix"], &[]),
    ("jsonnet", "Jsonnet", &["jsonnet", "libsonnet"], &[]),
    ("rego", "Rego", &["rego"], &[]),
    ("epp", "Puppet", &["epp"], &[]),
    ("qml", "QML", &["qml", "qmlproject"], &[]),
    ("robot", "Robot Framework", &["robot", "resource"], &[]),
    ("jq", "jq", &["jq"], &[]),
    (
        "Dockerfile",
        "Dockerfile",
        &["dockerfile", "containerfile"],
        &["dockerfile", "containerfile"],
    ),
    (
        "Makefile",
        "Makefile",
        &["mk", "mak", "make"],
        &["makefile", "gnumakefile", "bsdmakefile"],
    ),
    ("cmake", "CMake", &["cmake"], &["cmakelists.txt"]),
    ("ninja", "Ninja", &["ninja"], &[]),
    ("nsi", "NSIS", &["nsi", "nsh"], &[]),
    (
        "xml",
        "XML",
        &[
            "xml",
            "xsd",
            "xsl",
            "xslt",
            "xaml",
            "csproj",
            "fsproj",
            "vbproj",
            "vcxproj",
            "props",
            "targets",
            "resx",
            "nuspec",
            "plist",
            "storyboard",
            "xib",
            "wsdl",
            "rss",
            "atom",
            "opml",
            "kml",
            "gpx",
            "manifest",
            "xlf",
            "config",
            "pom",
            "fxml",
            "iml",
            "tld",
            "svg",
        ],
        &["pom.xml", "web.config", "app.config"],
    ),
    (
        "ini",
        "INI",
        &["ini", "cfg", "inf", "reg", "desktop", "url", "lng", "prefs"],
        &[
            ".editorconfig",
            ".coveragerc",
            ".pylintrc",
            ".flake8",
            "setup.cfg",
            "tox.ini",
            ".npmrc",
            ".yarnrc",
            "pytest.ini",
            "mypy.ini",
            ".hgrc",
            ".gitlint",
        ],
    ),
    ("properties", "Properties", &["properties"], &[]),
    (
        "nginx.conf",
        "Config",
        &["conf", "nginx"],
        &["nginx.conf", "mime.types"],
    ),
    (
        ".env",
        "Environment",
        &["env"],
        &[".env", ".envrc", ".flaskenv"],
    ),
    (
        "gitignore",
        "Ignore list",
        &[
            "gitignore",
            "dockerignore",
            "npmignore",
            "eslintignore",
            "prettierignore",
            "vscodeignore",
            "slugignore",
        ],
        &[
            ".gitignore",
            ".dockerignore",
            ".npmignore",
            ".eslintignore",
            ".prettierignore",
            ".vscodeignore",
            ".hgignore",
            ".gcloudignore",
            ".helmignore",
        ],
    ),
    (
        "gitattributes",
        "Git attributes",
        &["gitattributes"],
        &[".gitattributes"],
    ),
    (
        "gitconfig",
        "Git config",
        &["gitconfig"],
        &[".gitconfig", ".gitmodules"],
    ),
    (
        "COMMIT_EDITMSG",
        "Git commit",
        &[],
        &["commit_editmsg", "merge_msg", "tag_editmsg"],
    ),
    ("mailmap", "Git mailmap", &["mailmap"], &[".mailmap"]),
    ("csv", "CSV", &["csv", "tsv", "psv"], &[]),
    ("diff", "Diff", &["diff", "patch", "rej"], &[]),
    (
        "tex",
        "LaTeX",
        &["tex", "ltx", "sty", "cls", "dtx", "ins"],
        &[],
    ),
    ("bib", "BibTeX", &["bib"], &[]),
    ("rst", "reStructuredText", &["rst"], &[]),
    ("adoc", "AsciiDoc", &["adoc", "asciidoc", "asc"], &[]),
    ("org", "Org", &["org"], &[]),
    ("textile", "Textile", &["textile"], &[]),
    ("wiki", "MediaWiki", &["wiki", "mediawiki"], &[]),
    (
        "man",
        "Manual page",
        &[
            "man", "1", "2", "3", "4", "5", "6", "7", "8", "9", "groff", "troff",
        ],
        &[],
    ),
    ("log", "Log", &["log", "out"], &[]),
    ("http", "HTTP", &["http", "rest"], &[]),
    (
        "requirements.txt",
        "pip requirements",
        &[],
        &["requirements.txt", "requirements.in", "constraints.txt"],
    ),
    ("bat", "Batch", &["bat", "cmd", "btm"], &[]),
    ("bash", "PowerShell", &["ps1", "psm1", "psd1"], &[]),
    ("fish", "Fish", &["fish"], &[]),
    ("awk", "AWK", &["awk", "gawk", "mawk", "nawk"], &[]),
    (
        "vim",
        "Vim script",
        &["vim", "vimrc"],
        &[".vimrc", "_vimrc", ".gvimrc"],
    ),
    ("crontab", "Crontab", &["crontab"], &["crontab"]),
    ("ssh_config", "SSH config", &[], &["ssh_config"]),
    ("sshd_config", "SSHD config", &[], &["sshd_config"]),
    (
        "known_hosts",
        "Known hosts",
        &[],
        &["known_hosts", "authorized_keys"],
    ),
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

pub fn language_for(path: &Path, first_line: &str) -> (&'static str, &'static str) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if let Some(l) = LANGS.iter().find(|l| l.3.contains(&name.as_str())) {
        return (l.0, l.1);
    }
    if let Some(l) = crate::ext::native::language(&name, true) {
        return l;
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
    if let Some(l) = crate::ext::native::language(&name, false) {
        return l;
    }
    shebang(first_line).unwrap_or(("text", "Plain Text"))
}

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

pub fn file_icon_el(name: &str, size: gpui_kit::Pixels) -> gpui_kit::AnyElement {
    use gpui_kit::{
        component::{Icon, IconName},
        img, IntoElement, ObjectFit, Styled, StyledImage,
    };
    match crate::ext::vsx_icons::file_icon(name, crate::theme::palette().dark) {
        Some(crate::ext::vsx_icons::IconSource::Asset(uri)) => img(uri)
            .flex_none()
            .size(size)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        Some(crate::ext::vsx_icons::IconSource::File(path)) => img((*path).clone())
            .flex_none()
            .size(size)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        None => Icon::new(IconName::File)
            .flex_none()
            .size(size)
            .text_color(crate::theme::c(crate::theme::FG_MUTED))
            .into_any_element(),
    }
}

pub fn folder_icon_el(
    name: &str,
    open: bool,
    root: bool,
    size: gpui_kit::Pixels,
) -> gpui_kit::AnyElement {
    use gpui_kit::{
        component::{Icon, IconName},
        img, IntoElement, ObjectFit, Styled, StyledImage,
    };
    match crate::ext::vsx_icons::folder_icon(name, open, root, crate::theme::palette().dark) {
        Some(crate::ext::vsx_icons::IconSource::Asset(uri)) => img(uri)
            .flex_none()
            .size(size)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        Some(crate::ext::vsx_icons::IconSource::File(path)) => img((*path).clone())
            .flex_none()
            .size(size)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        None => Icon::new(if open {
            IconName::FolderOpen
        } else {
            IconName::Folder
        })
        .flex_none()
        .size(size)
        .text_color(crate::theme::c(crate::theme::FOLDER))
        .into_any_element(),
    }
}

gpui_kit::assets::icon_assets!(pub FileIcons, [
    File, FileCode, FileBraces, FileText, FileImage, FileLock, FileCog, FileTerminal, FileArchive,
    FileMusic, FileVideoCamera, FileType, FileSpreadsheet, FileKey, FolderGit, Clock,
    GitBranch, GitCommitHorizontal, GitPullRequest, CloudUpload, RefreshCcw, Rocket, FileDiff, Undo2,
    TextCursorInput, Brain, Pencil, AtSign, Paperclip, ShieldAlert, PencilLine, MessageSquareCode, MessagesSquare,
    Download, LoaderCircle, CircleQuestionMark, ExternalLink, ScrollText, FilePlus,
    Link,
    Lightbulb, Image,
    Blocks, Pickaxe, Box, Coffee, Braces, Hexagon, Hammer, Upload, Server, FlaskConical, Trash, Package,
    Sparkles, FolderPlus,
    Gauge, BadgeCheck, ShieldCheck, Timer, FilePen, Wrench,
    ListCollapse, GitFork, GitMerge, UserPlus,
    Leaf,
    Focus, Maximize2,
    TextAlignStart, Code, Shrink, FolderCheck,
    ListFilter, Archive, ArchiveRestore, Pin, PinOff,
    TextWrap, ZoomIn, ZoomOut
]);

#[cfg(test)]
mod tests {
    use super::{language_for, walk};
    use std::path::Path;

    #[test]
    fn material_icons_map_files_and_folders_by_default() {
        use crate::ext::vsx_icons::{self, IconSource};
        vsx_icons::set_installed(Vec::new());
        let check = |name: &str, expected_svg: &str| {
            let src = vsx_icons::file_icon(name, true).expect(name);
            let IconSource::Asset(uri) = src else {
                panic!("expected bundled material asset for {name}");
            };
            assert!(uri.ends_with(expected_svg), "{name} -> {uri}");
            let bytes = vsx_icons::load_bundled_asset(uri.as_ref()).expect("svg bytes");
            assert!(
                std::str::from_utf8(&bytes).unwrap().contains("<svg"),
                "{uri}"
            );
        };
        check("main.py", "python.svg");
        check("app.ts", "typescript.svg");
        check("types.d.ts", "typescript-def.svg");
        check("Button.test.tsx", "test-jsx.svg");
        check("README.md", "readme.svg");
        check("Dockerfile", "docker.svg");
        check(".env.local", "tune.svg");
        check("package.json", "nodejs.svg");
        check("Cargo.toml", "toml.svg");
        check("main.rs", "rust.svg");
        check("pack.mcaddon", "minecraft.svg");

        let src_dir = vsx_icons::folder_icon("src", false, false, true).expect("src folder");
        assert!(matches!(src_dir, IconSource::Asset(ref u) if u.ends_with("folder-src.svg")));
    }

    #[test]
    fn a_huge_folder_does_not_hide_its_siblings() {
        let root = std::env::temp_dir().join(format!("fluxcode-walk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("a-big")).unwrap();
        std::fs::create_dir_all(root.join("z-code")).unwrap();
        for i in 0..50 {
            std::fs::write(root.join("a-big").join(format!("f{i}.txt")), b"x").unwrap();
        }
        std::fs::write(root.join("z-code").join("main.rs"), b"x").unwrap();
        let mut budget = 10;
        let nodes = walk(&root, &mut budget);
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"z-code"), "{names:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_shallow_scan_lists_folders_without_reading_them() {
        let root = std::env::temp_dir().join(format!("fluxcode-shallow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub").join("deep.txt"), b"x").unwrap();
        std::fs::write(root.join("a.txt"), b"x").unwrap();
        let nodes = super::scan_shallow(&root);
        let sub = nodes.iter().find(|n| n.name == "sub").expect("sub");
        assert!(!sub.loaded);
        assert_eq!(sub.children.as_ref().map(Vec::len), Some(0));
        let file = nodes.iter().find(|n| n.name == "a.txt").expect("a.txt");
        assert!(file.loaded);
        assert!(file.children.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_remote_listing_has_the_same_shape() {
        let dir = Path::new(r"\\wsl.localhost\Ubuntu\home\me");
        let nodes =
            super::nodes_from_remote(dir, vec![("b.txt".into(), false), ("a-dir".into(), true)]);
        assert_eq!(nodes[0].name, "a-dir");
        assert!(nodes[0].children.is_some() && !nodes[0].loaded);
        assert_eq!(nodes[0].id, r"\\wsl.localhost\Ubuntu\home\me\a-dir");
        assert_eq!(nodes[1].name, "b.txt");
        assert!(nodes[1].children.is_none());
    }

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

    #[test]
    fn cmp_name_and_scan_paths_skip_build_dirs() {
        assert_eq!(
            super::cmp_name("Alpha.rs", "beta.rs"),
            std::cmp::Ordering::Less
        );
        assert_eq!(super::cmp_name("file", "file.rs"), std::cmp::Ordering::Less);
        let root = std::env::temp_dir().join(format!("fluxcode-scanpaths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("src/main.rs"), b"fn main() {}").unwrap();
        std::fs::write(root.join("node_modules/pkg/index.js"), b"x").unwrap();
        let got = super::scan_paths(&root, 100);
        assert_eq!(got.len(), 1);
        assert!(got[0].ends_with("main.rs"), "{got:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn merge_nodes_preserves_nested_loaded_children_in_multi_root() {
        use super::{merge_nodes, Node};
        let old = vec![Node {
            id: "/ws".into(),
            name: "ws".into(),
            loaded: true,
            children: Some(vec![Node {
                id: "/ws/a".into(),
                name: "a".into(),
                loaded: true,
                children: Some(vec![Node {
                    id: "/ws/a/f.js".into(),
                    name: "f.js".into(),
                    loaded: true,
                    children: None,
                }]),
            }]),
        }];
        let fresh = vec![Node {
            id: "/ws".into(),
            name: "ws".into(),
            loaded: true,
            children: Some(vec![
                Node {
                    id: "/ws/a".into(),
                    name: "a".into(),
                    loaded: false,
                    children: Some(Vec::new()),
                },
                Node {
                    id: "/ws/b".into(),
                    name: "b".into(),
                    loaded: false,
                    children: Some(Vec::new()),
                },
            ]),
        }];
        let merged = merge_nodes(old, fresh);
        let a = &merged[0].children.as_ref().unwrap()[0];
        assert!(a.loaded);
        assert_eq!(a.children.as_ref().unwrap().len(), 1);
        assert_eq!(a.children.as_ref().unwrap()[0].name, "f.js");
    }

    #[test]
    fn ancestor_dirs_lists_only_parents_within_root() {
        let roots = vec![std::path::PathBuf::from("C:/proj")];
        let dirs = super::ancestor_dirs(Path::new("C:/proj/a/b/c.js"), &roots);
        assert_eq!(dirs.len(), 2);
        assert!(dirs[0].ends_with("a"));
        assert!(dirs[1].ends_with("b"));
        let multi = vec![
            std::path::PathBuf::from("C:/p1"),
            std::path::PathBuf::from("C:/p2"),
        ];
        let dirs2 = super::ancestor_dirs(Path::new("C:/p2/sub/x.json"), &multi);
        assert_eq!(dirs2.len(), 2);
        assert!(dirs2[0].ends_with("p2"));
        assert!(dirs2[1].ends_with("sub"));
    }
}
