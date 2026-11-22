
use crate::fluxicon::{self, FluxIcon};
use gpui_kit::component::tree::TreeItem;
use std::path::Path;

pub(crate) const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "dist", "build", ".next", "__pycache__", ".venv", "venv",
    ".cache", ".local", ".cargo", ".rustup", ".npm", ".nvm", ".bun", ".pnpm-store", ".yarn",
    ".deno", ".volta", ".asdf", ".android", ".gradle", ".m2", ".vscode-server", ".ollama",
    ".oh-my-zsh", ".dotnet", ".nuget", ".conda", ".pyenv", ".sdkman", ".gem", ".pub-cache",
    ".stack", ".ghcup", ".cabal", ".terraform.d",
];
pub(crate) const MAX_ENTRIES: usize = 20_000;

#[derive(Clone)]
pub struct Node {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) children: Option<Vec<Node>>,
    pub(crate) loaded: bool,
}

pub fn is_image(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg" | "tif" | "tiff")
    )
}

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

pub fn nodes_from_remote(dir: &Path, entries: Vec<(String, bool)>) -> Vec<Node> {
    let mut entries = entries;
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    entries
        .into_iter()
        .map(|(name, is_dir)| {
            let id = dir.join(&name).to_string_lossy().into_owned();
            let id = if dir.as_os_str().to_string_lossy().starts_with(r"\\") { id.replace('/', r"\") } else { id };
            if is_dir {
                Node { id, name, children: Some(Vec::new()), loaded: false }
            } else {
                Node { id, name, children: None, loaded: true }
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
            name: r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| r.display().to_string()),
            children: Some(list(r)),
            loaded: true,
        })
        .collect()
}

pub fn scan_dir(dir: &Path) -> Vec<Node> {
    list(dir)
}

fn list(dir: &Path) -> Vec<Node> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<(bool, String, std::path::PathBuf)> = rd
        .flatten()
        .map(|e| {
            let p = e.path();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (is_dir, e.file_name().to_string_lossy().into_owned(), p)
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    entries
        .into_iter()
        .map(|(is_dir, name, path)| {
            let id = path.to_string_lossy().into_owned();
            if is_dir {
                Node { id, name, children: Some(Vec::new()), loaded: false }
            } else {
                Node { id, name, children: None, loaded: true }
            }
        })
        .collect()
}

pub fn scan_roots(roots: &[std::path::PathBuf]) -> Vec<Node> {
    let mut budget = MAX_ENTRIES;
    roots
        .iter()
        .map(|r| Node {
            id: r.to_string_lossy().into_owned(),
            name: r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| r.display().to_string()),
            children: Some(walk(r, &mut budget)),
            loaded: true,
        })
        .collect()
}

pub fn to_items(nodes: Vec<Node>, dirs: &mut std::collections::HashSet<String>, expanded: &std::collections::HashSet<String>) -> Vec<TreeItem> {
    nodes
        .into_iter()
        .map(|n| match n.children {
            Some(ch) => {
                dirs.insert(n.id.clone());
                let mut item = TreeItem::new(n.id.clone(), n.name);
                item = if n.loaded {
                    item.children(to_items(ch, dirs, expanded))
                } else {
                    item.child(TreeItem::new(format!("{}\u{1}", n.id), "…"))
                };
                if expanded.contains(&n.id) {
                    item = item.expanded(true);
                }
                item
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
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));

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
            out.push(Node { id, name, children: Some(Vec::new()), loaded: true });
        } else {
            out.push(Node { id, name, children: None, loaded: true });
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
    ("markdown", "Flux", &["flux"], &[]),
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

pub fn language_for(path: &Path, first_line: &str) -> (&'static str, &'static str) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
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


pub fn icon_for(name: &str) -> (FluxIcon, Option<u32>) {
    if let Some(hit) = fluxicon::for_name(name) {
        return hit;
    }
    match language_for(Path::new(&name.to_ascii_lowercase()), "").0 {
        "text" => (fluxicon::FILE, None),
        _ => (fluxicon::CODE, None),
    }
}

pub fn folder_icon(_name: &str, open: bool) -> FluxIcon {
    if open {
        fluxicon::FOLDER_OPEN
    } else {
        fluxicon::FOLDER
    }
}

gpui_kit::assets::icon_assets!(pub FileIcons, [
    File, FileCode, FileBraces, FileText, FileImage, FileLock, FileCog, FileTerminal, FileArchive,
    FileMusic, FileVideoCamera, FileType, FileSpreadsheet, FileKey, FolderGit, Clock,
    GitBranch, GitCommitHorizontal, GitPullRequest, CloudUpload, RefreshCcw, Rocket, FileDiff, Undo2,
    TextCursorInput, Brain, Pencil, AtSign, Paperclip, ShieldAlert, PencilLine, MessageSquareCode, MessagesSquare,
    Download, LoaderCircle, CircleQuestionMark, ExternalLink, ScrollText, FilePlus,
    Users, LogIn, House, Router, RadioTower, Link,
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
    fn typed_names_map_to_their_file_icons() {
        use crate::fluxicon;
        assert_eq!(super::icon_for("main.py"), (fluxicon::PYTHON, Some(0x4F9FD8)));
        assert_eq!(super::icon_for("app.ts").0, fluxicon::TYPESCRIPT);
        assert_eq!(super::icon_for("types.d.ts").0, fluxicon::TS_DECL);
        assert_eq!(super::icon_for("Button.test.tsx").0, fluxicon::TEST_TS);
        assert_eq!(super::icon_for("plain.txt").0, fluxicon::TEXT);
        assert_eq!(super::icon_for("README.md").0, fluxicon::README);
        assert_eq!(super::icon_for("Dockerfile.prod").0, fluxicon::DOCKER);
        assert_eq!(super::icon_for(".env.local").0, fluxicon::ENV);
        assert_eq!(super::icon_for("package.json").0, fluxicon::NPM);
        assert_eq!(super::icon_for("package-lock.json").0, fluxicon::LOCK);
        assert_eq!(super::icon_for("vite.config.ts").0, fluxicon::TOOL_CONFIG);
        assert_eq!(super::icon_for("schema.prisma").0, fluxicon::PRISMA);
        assert_eq!(super::icon_for("data.sqlite").0, fluxicon::SQLITE);
        assert_eq!(super::icon_for("query.sql").0, fluxicon::SQL);
        assert_eq!(super::icon_for("requirements-dev.txt").0, fluxicon::PYPACKAGE);
        assert_eq!(super::icon_for("icon-48.png").0, fluxicon::IMAGE);
        assert_eq!(super::icon_for("x.sml").0, fluxicon::CODE);
        assert_eq!(super::icon_for("NOTES").0, fluxicon::FILE);
    }

    #[test]
    fn every_fluxicon_is_embedded_and_drawn() {
        for (path, svg) in crate::fluxicon::SVGS {
            let text = std::str::from_utf8(svg).unwrap();
            assert!(text.starts_with("<svg") && text.contains("<path"), "{path}");
        }
        assert_eq!(crate::fluxicon::SVGS.len(), crate::fluxicon::KINDS);
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
        let nodes = super::nodes_from_remote(dir, vec![("b.txt".into(), false), ("a-dir".into(), true)]);
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
}
