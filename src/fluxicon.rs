
use gpui_kit::SharedString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FluxIcon(pub &'static str);

impl gpui_kit::assets::IconNamed for FluxIcon {
    fn path(self) -> SharedString {
        self.0.into()
    }
}

pub const PYTHON: FluxIcon = FluxIcon("fluxicon/python.svg");
pub const JAVASCRIPT: FluxIcon = FluxIcon("fluxicon/javascript.svg");
pub const TYPESCRIPT: FluxIcon = FluxIcon("fluxicon/typescript.svg");
pub const TS_DECL: FluxIcon = FluxIcon("fluxicon/ts-decl.svg");
pub const JSX: FluxIcon = FluxIcon("fluxicon/jsx.svg");
pub const TSX: FluxIcon = FluxIcon("fluxicon/tsx.svg");
pub const RUST: FluxIcon = FluxIcon("fluxicon/rust.svg");
pub const GO: FluxIcon = FluxIcon("fluxicon/go.svg");
pub const C: FluxIcon = FluxIcon("fluxicon/c.svg");
pub const C_HEADER: FluxIcon = FluxIcon("fluxicon/c-header.svg");
pub const CPP: FluxIcon = FluxIcon("fluxicon/cpp.svg");
pub const CPP_HEADER: FluxIcon = FluxIcon("fluxicon/cpp-header.svg");
pub const CSHARP: FluxIcon = FluxIcon("fluxicon/csharp.svg");
pub const FSHARP: FluxIcon = FluxIcon("fluxicon/fsharp.svg");
pub const VB: FluxIcon = FluxIcon("fluxicon/vb.svg");
pub const JAVA: FluxIcon = FluxIcon("fluxicon/java.svg");
pub const CLASS: FluxIcon = FluxIcon("fluxicon/class.svg");
pub const KOTLIN: FluxIcon = FluxIcon("fluxicon/kotlin.svg");
pub const SCALA: FluxIcon = FluxIcon("fluxicon/scala.svg");
pub const GROOVY: FluxIcon = FluxIcon("fluxicon/groovy.svg");
pub const CLOJURE: FluxIcon = FluxIcon("fluxicon/clojure.svg");
pub const SWIFT: FluxIcon = FluxIcon("fluxicon/swift.svg");
pub const OBJC: FluxIcon = FluxIcon("fluxicon/objc.svg");
pub const DART: FluxIcon = FluxIcon("fluxicon/dart.svg");
pub const RUBY: FluxIcon = FluxIcon("fluxicon/ruby.svg");
pub const PHP: FluxIcon = FluxIcon("fluxicon/php.svg");
pub const PERL: FluxIcon = FluxIcon("fluxicon/perl.svg");
pub const RAKU: FluxIcon = FluxIcon("fluxicon/raku.svg");
pub const LUA: FluxIcon = FluxIcon("fluxicon/lua.svg");
pub const R: FluxIcon = FluxIcon("fluxicon/r.svg");
pub const JULIA: FluxIcon = FluxIcon("fluxicon/julia.svg");
pub const HASKELL: FluxIcon = FluxIcon("fluxicon/haskell.svg");
pub const OCAML: FluxIcon = FluxIcon("fluxicon/ocaml.svg");
pub const REASON: FluxIcon = FluxIcon("fluxicon/reason.svg");
pub const PURESCRIPT: FluxIcon = FluxIcon("fluxicon/purescript.svg");
pub const ELM: FluxIcon = FluxIcon("fluxicon/elm.svg");
pub const ERLANG: FluxIcon = FluxIcon("fluxicon/erlang.svg");
pub const ELIXIR: FluxIcon = FluxIcon("fluxicon/elixir.svg");
pub const GLEAM: FluxIcon = FluxIcon("fluxicon/gleam.svg");
pub const ZIG: FluxIcon = FluxIcon("fluxicon/zig.svg");
pub const NIM: FluxIcon = FluxIcon("fluxicon/nim.svg");
pub const CRYSTAL: FluxIcon = FluxIcon("fluxicon/crystal.svg");
pub const D: FluxIcon = FluxIcon("fluxicon/d.svg");
pub const V: FluxIcon = FluxIcon("fluxicon/v.svg");
pub const ODIN: FluxIcon = FluxIcon("fluxicon/odin.svg");
pub const HAXE: FluxIcon = FluxIcon("fluxicon/haxe.svg");
pub const MOJO: FluxIcon = FluxIcon("fluxicon/mojo.svg");
pub const FORTRAN: FluxIcon = FluxIcon("fluxicon/fortran.svg");
pub const COBOL: FluxIcon = FluxIcon("fluxicon/cobol.svg");
pub const PASCAL: FluxIcon = FluxIcon("fluxicon/pascal.svg");
pub const ADA: FluxIcon = FluxIcon("fluxicon/ada.svg");
pub const LISP: FluxIcon = FluxIcon("fluxicon/lisp.svg");
pub const PROLOG: FluxIcon = FluxIcon("fluxicon/prolog.svg");
pub const ASM: FluxIcon = FluxIcon("fluxicon/asm.svg");
pub const WASM: FluxIcon = FluxIcon("fluxicon/wasm.svg");
pub const SOLIDITY: FluxIcon = FluxIcon("fluxicon/solidity.svg");
pub const CUDA: FluxIcon = FluxIcon("fluxicon/cuda.svg");
pub const SHADER: FluxIcon = FluxIcon("fluxicon/shader.svg");
pub const VERILOG: FluxIcon = FluxIcon("fluxicon/verilog.svg");
pub const ARDUINO: FluxIcon = FluxIcon("fluxicon/arduino.svg");
pub const GDSCRIPT: FluxIcon = FluxIcon("fluxicon/gdscript.svg");
pub const COFFEE: FluxIcon = FluxIcon("fluxicon/coffee.svg");
pub const APEX: FluxIcon = FluxIcon("fluxicon/apex.svg");
pub const QSHARP: FluxIcon = FluxIcon("fluxicon/qsharp.svg");
pub const WOLFRAM: FluxIcon = FluxIcon("fluxicon/wolfram.svg");
pub const MATLAB: FluxIcon = FluxIcon("fluxicon/matlab.svg");
pub const TCL: FluxIcon = FluxIcon("fluxicon/tcl.svg");
pub const VIM: FluxIcon = FluxIcon("fluxicon/vim.svg");
pub const APPLESCRIPT: FluxIcon = FluxIcon("fluxicon/applescript.svg");
pub const AUTOHOTKEY: FluxIcon = FluxIcon("fluxicon/autohotkey.svg");
pub const SHELL: FluxIcon = FluxIcon("fluxicon/shell.svg");
pub const POWERSHELL: FluxIcon = FluxIcon("fluxicon/powershell.svg");
pub const BATCH: FluxIcon = FluxIcon("fluxicon/batch.svg");
pub const HTML: FluxIcon = FluxIcon("fluxicon/html.svg");
pub const CSS: FluxIcon = FluxIcon("fluxicon/css.svg");
pub const SCSS: FluxIcon = FluxIcon("fluxicon/scss.svg");
pub const LESS: FluxIcon = FluxIcon("fluxicon/less.svg");
pub const STYLUS: FluxIcon = FluxIcon("fluxicon/stylus.svg");
pub const VUE: FluxIcon = FluxIcon("fluxicon/vue.svg");
pub const SVELTE: FluxIcon = FluxIcon("fluxicon/svelte.svg");
pub const ASTRO: FluxIcon = FluxIcon("fluxicon/astro.svg");
pub const HANDLEBARS: FluxIcon = FluxIcon("fluxicon/handlebars.svg");
pub const PUG: FluxIcon = FluxIcon("fluxicon/pug.svg");
pub const HAML: FluxIcon = FluxIcon("fluxicon/haml.svg");
pub const EJS: FluxIcon = FluxIcon("fluxicon/ejs.svg");
pub const LIQUID: FluxIcon = FluxIcon("fluxicon/liquid.svg");
pub const TWIG: FluxIcon = FluxIcon("fluxicon/twig.svg");
pub const RAZOR: FluxIcon = FluxIcon("fluxicon/razor.svg");
pub const JINJA: FluxIcon = FluxIcon("fluxicon/jinja.svg");
pub const GRAPHQL: FluxIcon = FluxIcon("fluxicon/graphql.svg");
pub const MARKDOWN: FluxIcon = FluxIcon("fluxicon/markdown.svg");
pub const RST: FluxIcon = FluxIcon("fluxicon/rst.svg");
pub const ASCIIDOC: FluxIcon = FluxIcon("fluxicon/asciidoc.svg");
pub const LATEX: FluxIcon = FluxIcon("fluxicon/latex.svg");
pub const TYPST: FluxIcon = FluxIcon("fluxicon/typst.svg");
pub const ORG: FluxIcon = FluxIcon("fluxicon/org.svg");
pub const TEXT: FluxIcon = FluxIcon("fluxicon/text.svg");
pub const FLUX: FluxIcon = FluxIcon("fluxicon/flux.svg");
pub const PDF: FluxIcon = FluxIcon("fluxicon/pdf.svg");
pub const DOC: FluxIcon = FluxIcon("fluxicon/doc.svg");
pub const SHEET: FluxIcon = FluxIcon("fluxicon/sheet.svg");
pub const SLIDES: FluxIcon = FluxIcon("fluxicon/slides.svg");
pub const EBOOK: FluxIcon = FluxIcon("fluxicon/ebook.svg");
pub const NOTEBOOK: FluxIcon = FluxIcon("fluxicon/notebook.svg");
pub const DIFF: FluxIcon = FluxIcon("fluxicon/diff.svg");
pub const LOG: FluxIcon = FluxIcon("fluxicon/log.svg");
pub const JSON: FluxIcon = FluxIcon("fluxicon/json.svg");
pub const YAML: FluxIcon = FluxIcon("fluxicon/yaml.svg");
pub const TOML: FluxIcon = FluxIcon("fluxicon/toml.svg");
pub const INI: FluxIcon = FluxIcon("fluxicon/ini.svg");
pub const XML: FluxIcon = FluxIcon("fluxicon/xml.svg");
pub const CSV: FluxIcon = FluxIcon("fluxicon/csv.svg");
pub const COLUMNAR: FluxIcon = FluxIcon("fluxicon/columnar.svg");
pub const PROTO: FluxIcon = FluxIcon("fluxicon/proto.svg");
pub const TERRAFORM: FluxIcon = FluxIcon("fluxicon/terraform.svg");
pub const NIX: FluxIcon = FluxIcon("fluxicon/nix.svg");
pub const BICEP: FluxIcon = FluxIcon("fluxicon/bicep.svg");
pub const ENV: FluxIcon = FluxIcon("fluxicon/env.svg");
pub const SQL: FluxIcon = FluxIcon("fluxicon/sql.svg");
pub const SQLITE: FluxIcon = FluxIcon("fluxicon/sqlite.svg");
pub const DB_FILE: FluxIcon = FluxIcon("fluxicon/db-file.svg");
pub const PRISMA: FluxIcon = FluxIcon("fluxicon/prisma.svg");
pub const MONGO: FluxIcon = FluxIcon("fluxicon/mongo.svg");
pub const IMAGE: FluxIcon = FluxIcon("fluxicon/image.svg");
pub const SVG: FluxIcon = FluxIcon("fluxicon/svg.svg");
pub const VIDEO: FluxIcon = FluxIcon("fluxicon/video.svg");
pub const AUDIO: FluxIcon = FluxIcon("fluxicon/audio.svg");
pub const FONT: FluxIcon = FluxIcon("fluxicon/font.svg");
pub const MODEL3D: FluxIcon = FluxIcon("fluxicon/model3d.svg");
pub const BINARY: FluxIcon = FluxIcon("fluxicon/binary.svg");
pub const ARCHIVE: FluxIcon = FluxIcon("fluxicon/archive.svg");
pub const LOCK: FluxIcon = FluxIcon("fluxicon/lock.svg");
pub const CERT: FluxIcon = FluxIcon("fluxicon/cert.svg");
pub const KEY: FluxIcon = FluxIcon("fluxicon/key.svg");
pub const GIT: FluxIcon = FluxIcon("fluxicon/git.svg");
pub const DOCKER: FluxIcon = FluxIcon("fluxicon/docker.svg");
pub const NPM: FluxIcon = FluxIcon("fluxicon/npm.svg");
pub const CARGO: FluxIcon = FluxIcon("fluxicon/cargo.svg");
pub const PYPACKAGE: FluxIcon = FluxIcon("fluxicon/pypackage.svg");
pub const GOMOD: FluxIcon = FluxIcon("fluxicon/gomod.svg");
pub const GEM: FluxIcon = FluxIcon("fluxicon/gem.svg");
pub const COMPOSER: FluxIcon = FluxIcon("fluxicon/composer.svg");
pub const MAVEN: FluxIcon = FluxIcon("fluxicon/maven.svg");
pub const GRADLE: FluxIcon = FluxIcon("fluxicon/gradle.svg");
pub const DOTNET: FluxIcon = FluxIcon("fluxicon/dotnet.svg");
pub const PUBSPEC: FluxIcon = FluxIcon("fluxicon/pubspec.svg");
pub const SWIFTPKG: FluxIcon = FluxIcon("fluxicon/swiftpkg.svg");
pub const MIX: FluxIcon = FluxIcon("fluxicon/mix.svg");
pub const CABAL: FluxIcon = FluxIcon("fluxicon/cabal.svg");
pub const BUILD: FluxIcon = FluxIcon("fluxicon/build.svg");
pub const TSCONFIG: FluxIcon = FluxIcon("fluxicon/tsconfig.svg");
pub const TOOL_CONFIG: FluxIcon = FluxIcon("fluxicon/tool-config.svg");
pub const README: FluxIcon = FluxIcon("fluxicon/readme.svg");
pub const LICENSE: FluxIcon = FluxIcon("fluxicon/license.svg");
pub const CHANGELOG: FluxIcon = FluxIcon("fluxicon/changelog.svg");
pub const CONTRIBUTING: FluxIcon = FluxIcon("fluxicon/contributing.svg");
pub const TEST_JS: FluxIcon = FluxIcon("fluxicon/test-js.svg");
pub const TEST_TS: FluxIcon = FluxIcon("fluxicon/test-ts.svg");
pub const TEST_PY: FluxIcon = FluxIcon("fluxicon/test-py.svg");
pub const TEST_GO: FluxIcon = FluxIcon("fluxicon/test-go.svg");
pub const TEST_RB: FluxIcon = FluxIcon("fluxicon/test-rb.svg");
pub const TEST_RS: FluxIcon = FluxIcon("fluxicon/test-rs.svg");
pub const FILE: FluxIcon = FluxIcon("fluxicon/file.svg");
pub const CODE: FluxIcon = FluxIcon("fluxicon/code.svg");
pub const FOLDER: FluxIcon = FluxIcon("fluxicon/folder.svg");
pub const FOLDER_OPEN: FluxIcon = FluxIcon("fluxicon/folder-open.svg");

pub static SVGS: &[(&str, &[u8])] = &[
    ("fluxicon/python.svg", include_bytes!("../assets/fluxicon/python.svg")),
    ("fluxicon/javascript.svg", include_bytes!("../assets/fluxicon/javascript.svg")),
    ("fluxicon/typescript.svg", include_bytes!("../assets/fluxicon/typescript.svg")),
    ("fluxicon/ts-decl.svg", include_bytes!("../assets/fluxicon/ts-decl.svg")),
    ("fluxicon/jsx.svg", include_bytes!("../assets/fluxicon/jsx.svg")),
    ("fluxicon/tsx.svg", include_bytes!("../assets/fluxicon/tsx.svg")),
    ("fluxicon/rust.svg", include_bytes!("../assets/fluxicon/rust.svg")),
    ("fluxicon/go.svg", include_bytes!("../assets/fluxicon/go.svg")),
    ("fluxicon/c.svg", include_bytes!("../assets/fluxicon/c.svg")),
    ("fluxicon/c-header.svg", include_bytes!("../assets/fluxicon/c-header.svg")),
    ("fluxicon/cpp.svg", include_bytes!("../assets/fluxicon/cpp.svg")),
    ("fluxicon/cpp-header.svg", include_bytes!("../assets/fluxicon/cpp-header.svg")),
    ("fluxicon/csharp.svg", include_bytes!("../assets/fluxicon/csharp.svg")),
    ("fluxicon/fsharp.svg", include_bytes!("../assets/fluxicon/fsharp.svg")),
    ("fluxicon/vb.svg", include_bytes!("../assets/fluxicon/vb.svg")),
    ("fluxicon/java.svg", include_bytes!("../assets/fluxicon/java.svg")),
    ("fluxicon/class.svg", include_bytes!("../assets/fluxicon/class.svg")),
    ("fluxicon/kotlin.svg", include_bytes!("../assets/fluxicon/kotlin.svg")),
    ("fluxicon/scala.svg", include_bytes!("../assets/fluxicon/scala.svg")),
    ("fluxicon/groovy.svg", include_bytes!("../assets/fluxicon/groovy.svg")),
    ("fluxicon/clojure.svg", include_bytes!("../assets/fluxicon/clojure.svg")),
    ("fluxicon/swift.svg", include_bytes!("../assets/fluxicon/swift.svg")),
    ("fluxicon/objc.svg", include_bytes!("../assets/fluxicon/objc.svg")),
    ("fluxicon/dart.svg", include_bytes!("../assets/fluxicon/dart.svg")),
    ("fluxicon/ruby.svg", include_bytes!("../assets/fluxicon/ruby.svg")),
    ("fluxicon/php.svg", include_bytes!("../assets/fluxicon/php.svg")),
    ("fluxicon/perl.svg", include_bytes!("../assets/fluxicon/perl.svg")),
    ("fluxicon/raku.svg", include_bytes!("../assets/fluxicon/raku.svg")),
    ("fluxicon/lua.svg", include_bytes!("../assets/fluxicon/lua.svg")),
    ("fluxicon/r.svg", include_bytes!("../assets/fluxicon/r.svg")),
    ("fluxicon/julia.svg", include_bytes!("../assets/fluxicon/julia.svg")),
    ("fluxicon/haskell.svg", include_bytes!("../assets/fluxicon/haskell.svg")),
    ("fluxicon/ocaml.svg", include_bytes!("../assets/fluxicon/ocaml.svg")),
    ("fluxicon/reason.svg", include_bytes!("../assets/fluxicon/reason.svg")),
    ("fluxicon/purescript.svg", include_bytes!("../assets/fluxicon/purescript.svg")),
    ("fluxicon/elm.svg", include_bytes!("../assets/fluxicon/elm.svg")),
    ("fluxicon/erlang.svg", include_bytes!("../assets/fluxicon/erlang.svg")),
    ("fluxicon/elixir.svg", include_bytes!("../assets/fluxicon/elixir.svg")),
    ("fluxicon/gleam.svg", include_bytes!("../assets/fluxicon/gleam.svg")),
    ("fluxicon/zig.svg", include_bytes!("../assets/fluxicon/zig.svg")),
    ("fluxicon/nim.svg", include_bytes!("../assets/fluxicon/nim.svg")),
    ("fluxicon/crystal.svg", include_bytes!("../assets/fluxicon/crystal.svg")),
    ("fluxicon/d.svg", include_bytes!("../assets/fluxicon/d.svg")),
    ("fluxicon/v.svg", include_bytes!("../assets/fluxicon/v.svg")),
    ("fluxicon/odin.svg", include_bytes!("../assets/fluxicon/odin.svg")),
    ("fluxicon/haxe.svg", include_bytes!("../assets/fluxicon/haxe.svg")),
    ("fluxicon/mojo.svg", include_bytes!("../assets/fluxicon/mojo.svg")),
    ("fluxicon/fortran.svg", include_bytes!("../assets/fluxicon/fortran.svg")),
    ("fluxicon/cobol.svg", include_bytes!("../assets/fluxicon/cobol.svg")),
    ("fluxicon/pascal.svg", include_bytes!("../assets/fluxicon/pascal.svg")),
    ("fluxicon/ada.svg", include_bytes!("../assets/fluxicon/ada.svg")),
    ("fluxicon/lisp.svg", include_bytes!("../assets/fluxicon/lisp.svg")),
    ("fluxicon/prolog.svg", include_bytes!("../assets/fluxicon/prolog.svg")),
    ("fluxicon/asm.svg", include_bytes!("../assets/fluxicon/asm.svg")),
    ("fluxicon/wasm.svg", include_bytes!("../assets/fluxicon/wasm.svg")),
    ("fluxicon/solidity.svg", include_bytes!("../assets/fluxicon/solidity.svg")),
    ("fluxicon/cuda.svg", include_bytes!("../assets/fluxicon/cuda.svg")),
    ("fluxicon/shader.svg", include_bytes!("../assets/fluxicon/shader.svg")),
    ("fluxicon/verilog.svg", include_bytes!("../assets/fluxicon/verilog.svg")),
    ("fluxicon/arduino.svg", include_bytes!("../assets/fluxicon/arduino.svg")),
    ("fluxicon/gdscript.svg", include_bytes!("../assets/fluxicon/gdscript.svg")),
    ("fluxicon/coffee.svg", include_bytes!("../assets/fluxicon/coffee.svg")),
    ("fluxicon/apex.svg", include_bytes!("../assets/fluxicon/apex.svg")),
    ("fluxicon/qsharp.svg", include_bytes!("../assets/fluxicon/qsharp.svg")),
    ("fluxicon/wolfram.svg", include_bytes!("../assets/fluxicon/wolfram.svg")),
    ("fluxicon/matlab.svg", include_bytes!("../assets/fluxicon/matlab.svg")),
    ("fluxicon/tcl.svg", include_bytes!("../assets/fluxicon/tcl.svg")),
    ("fluxicon/vim.svg", include_bytes!("../assets/fluxicon/vim.svg")),
    ("fluxicon/applescript.svg", include_bytes!("../assets/fluxicon/applescript.svg")),
    ("fluxicon/autohotkey.svg", include_bytes!("../assets/fluxicon/autohotkey.svg")),
    ("fluxicon/shell.svg", include_bytes!("../assets/fluxicon/shell.svg")),
    ("fluxicon/powershell.svg", include_bytes!("../assets/fluxicon/powershell.svg")),
    ("fluxicon/batch.svg", include_bytes!("../assets/fluxicon/batch.svg")),
    ("fluxicon/html.svg", include_bytes!("../assets/fluxicon/html.svg")),
    ("fluxicon/css.svg", include_bytes!("../assets/fluxicon/css.svg")),
    ("fluxicon/scss.svg", include_bytes!("../assets/fluxicon/scss.svg")),
    ("fluxicon/less.svg", include_bytes!("../assets/fluxicon/less.svg")),
    ("fluxicon/stylus.svg", include_bytes!("../assets/fluxicon/stylus.svg")),
    ("fluxicon/vue.svg", include_bytes!("../assets/fluxicon/vue.svg")),
    ("fluxicon/svelte.svg", include_bytes!("../assets/fluxicon/svelte.svg")),
    ("fluxicon/astro.svg", include_bytes!("../assets/fluxicon/astro.svg")),
    ("fluxicon/handlebars.svg", include_bytes!("../assets/fluxicon/handlebars.svg")),
    ("fluxicon/pug.svg", include_bytes!("../assets/fluxicon/pug.svg")),
    ("fluxicon/haml.svg", include_bytes!("../assets/fluxicon/haml.svg")),
    ("fluxicon/ejs.svg", include_bytes!("../assets/fluxicon/ejs.svg")),
    ("fluxicon/liquid.svg", include_bytes!("../assets/fluxicon/liquid.svg")),
    ("fluxicon/twig.svg", include_bytes!("../assets/fluxicon/twig.svg")),
    ("fluxicon/razor.svg", include_bytes!("../assets/fluxicon/razor.svg")),
    ("fluxicon/jinja.svg", include_bytes!("../assets/fluxicon/jinja.svg")),
    ("fluxicon/graphql.svg", include_bytes!("../assets/fluxicon/graphql.svg")),
    ("fluxicon/markdown.svg", include_bytes!("../assets/fluxicon/markdown.svg")),
    ("fluxicon/rst.svg", include_bytes!("../assets/fluxicon/rst.svg")),
    ("fluxicon/asciidoc.svg", include_bytes!("../assets/fluxicon/asciidoc.svg")),
    ("fluxicon/latex.svg", include_bytes!("../assets/fluxicon/latex.svg")),
    ("fluxicon/typst.svg", include_bytes!("../assets/fluxicon/typst.svg")),
    ("fluxicon/org.svg", include_bytes!("../assets/fluxicon/org.svg")),
    ("fluxicon/text.svg", include_bytes!("../assets/fluxicon/text.svg")),
    ("fluxicon/flux.svg", include_bytes!("../assets/fluxicon/flux.svg")),
    ("fluxicon/pdf.svg", include_bytes!("../assets/fluxicon/pdf.svg")),
    ("fluxicon/doc.svg", include_bytes!("../assets/fluxicon/doc.svg")),
    ("fluxicon/sheet.svg", include_bytes!("../assets/fluxicon/sheet.svg")),
    ("fluxicon/slides.svg", include_bytes!("../assets/fluxicon/slides.svg")),
    ("fluxicon/ebook.svg", include_bytes!("../assets/fluxicon/ebook.svg")),
    ("fluxicon/notebook.svg", include_bytes!("../assets/fluxicon/notebook.svg")),
    ("fluxicon/diff.svg", include_bytes!("../assets/fluxicon/diff.svg")),
    ("fluxicon/log.svg", include_bytes!("../assets/fluxicon/log.svg")),
    ("fluxicon/json.svg", include_bytes!("../assets/fluxicon/json.svg")),
    ("fluxicon/yaml.svg", include_bytes!("../assets/fluxicon/yaml.svg")),
    ("fluxicon/toml.svg", include_bytes!("../assets/fluxicon/toml.svg")),
    ("fluxicon/ini.svg", include_bytes!("../assets/fluxicon/ini.svg")),
    ("fluxicon/xml.svg", include_bytes!("../assets/fluxicon/xml.svg")),
    ("fluxicon/csv.svg", include_bytes!("../assets/fluxicon/csv.svg")),
    ("fluxicon/columnar.svg", include_bytes!("../assets/fluxicon/columnar.svg")),
    ("fluxicon/proto.svg", include_bytes!("../assets/fluxicon/proto.svg")),
    ("fluxicon/terraform.svg", include_bytes!("../assets/fluxicon/terraform.svg")),
    ("fluxicon/nix.svg", include_bytes!("../assets/fluxicon/nix.svg")),
    ("fluxicon/bicep.svg", include_bytes!("../assets/fluxicon/bicep.svg")),
    ("fluxicon/env.svg", include_bytes!("../assets/fluxicon/env.svg")),
    ("fluxicon/sql.svg", include_bytes!("../assets/fluxicon/sql.svg")),
    ("fluxicon/sqlite.svg", include_bytes!("../assets/fluxicon/sqlite.svg")),
    ("fluxicon/db-file.svg", include_bytes!("../assets/fluxicon/db-file.svg")),
    ("fluxicon/prisma.svg", include_bytes!("../assets/fluxicon/prisma.svg")),
    ("fluxicon/mongo.svg", include_bytes!("../assets/fluxicon/mongo.svg")),
    ("fluxicon/image.svg", include_bytes!("../assets/fluxicon/image.svg")),
    ("fluxicon/svg.svg", include_bytes!("../assets/fluxicon/svg.svg")),
    ("fluxicon/video.svg", include_bytes!("../assets/fluxicon/video.svg")),
    ("fluxicon/audio.svg", include_bytes!("../assets/fluxicon/audio.svg")),
    ("fluxicon/font.svg", include_bytes!("../assets/fluxicon/font.svg")),
    ("fluxicon/model3d.svg", include_bytes!("../assets/fluxicon/model3d.svg")),
    ("fluxicon/binary.svg", include_bytes!("../assets/fluxicon/binary.svg")),
    ("fluxicon/archive.svg", include_bytes!("../assets/fluxicon/archive.svg")),
    ("fluxicon/lock.svg", include_bytes!("../assets/fluxicon/lock.svg")),
    ("fluxicon/cert.svg", include_bytes!("../assets/fluxicon/cert.svg")),
    ("fluxicon/key.svg", include_bytes!("../assets/fluxicon/key.svg")),
    ("fluxicon/git.svg", include_bytes!("../assets/fluxicon/git.svg")),
    ("fluxicon/docker.svg", include_bytes!("../assets/fluxicon/docker.svg")),
    ("fluxicon/npm.svg", include_bytes!("../assets/fluxicon/npm.svg")),
    ("fluxicon/cargo.svg", include_bytes!("../assets/fluxicon/cargo.svg")),
    ("fluxicon/pypackage.svg", include_bytes!("../assets/fluxicon/pypackage.svg")),
    ("fluxicon/gomod.svg", include_bytes!("../assets/fluxicon/gomod.svg")),
    ("fluxicon/gem.svg", include_bytes!("../assets/fluxicon/gem.svg")),
    ("fluxicon/composer.svg", include_bytes!("../assets/fluxicon/composer.svg")),
    ("fluxicon/maven.svg", include_bytes!("../assets/fluxicon/maven.svg")),
    ("fluxicon/gradle.svg", include_bytes!("../assets/fluxicon/gradle.svg")),
    ("fluxicon/dotnet.svg", include_bytes!("../assets/fluxicon/dotnet.svg")),
    ("fluxicon/pubspec.svg", include_bytes!("../assets/fluxicon/pubspec.svg")),
    ("fluxicon/swiftpkg.svg", include_bytes!("../assets/fluxicon/swiftpkg.svg")),
    ("fluxicon/mix.svg", include_bytes!("../assets/fluxicon/mix.svg")),
    ("fluxicon/cabal.svg", include_bytes!("../assets/fluxicon/cabal.svg")),
    ("fluxicon/build.svg", include_bytes!("../assets/fluxicon/build.svg")),
    ("fluxicon/tsconfig.svg", include_bytes!("../assets/fluxicon/tsconfig.svg")),
    ("fluxicon/tool-config.svg", include_bytes!("../assets/fluxicon/tool-config.svg")),
    ("fluxicon/readme.svg", include_bytes!("../assets/fluxicon/readme.svg")),
    ("fluxicon/license.svg", include_bytes!("../assets/fluxicon/license.svg")),
    ("fluxicon/changelog.svg", include_bytes!("../assets/fluxicon/changelog.svg")),
    ("fluxicon/contributing.svg", include_bytes!("../assets/fluxicon/contributing.svg")),
    ("fluxicon/test-js.svg", include_bytes!("../assets/fluxicon/test-js.svg")),
    ("fluxicon/test-ts.svg", include_bytes!("../assets/fluxicon/test-ts.svg")),
    ("fluxicon/test-py.svg", include_bytes!("../assets/fluxicon/test-py.svg")),
    ("fluxicon/test-go.svg", include_bytes!("../assets/fluxicon/test-go.svg")),
    ("fluxicon/test-rb.svg", include_bytes!("../assets/fluxicon/test-rb.svg")),
    ("fluxicon/test-rs.svg", include_bytes!("../assets/fluxicon/test-rs.svg")),
    ("fluxicon/file.svg", include_bytes!("../assets/fluxicon/file.svg")),
    ("fluxicon/code.svg", include_bytes!("../assets/fluxicon/code.svg")),
    ("fluxicon/folder.svg", include_bytes!("../assets/fluxicon/folder.svg")),
    ("fluxicon/folder-open.svg", include_bytes!("../assets/fluxicon/folder-open.svg")),
];

pub fn load(path: &str) -> Option<&'static [u8]> {
    if !path.starts_with("fluxicon/") {
        return None;
    }
    SVGS.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)
}

pub fn paths() -> impl Iterator<Item = &'static str> {
    SVGS.iter().map(|(p, _)| *p)
}

fn by_name(name: &str) -> Option<(FluxIcon, Option<u32>)> {
    let hit = match name {
        "setup.py" => (PYTHON, Some(0x4F9FD8)),
        "jenkinsfile" => (GROOVY, Some(0x4298B8)),
        "rakefile" => (RUBY, Some(0xE0464B)),
        "guardfile" => (RUBY, Some(0xE0464B)),
        "vagrantfile" => (RUBY, Some(0xE0464B)),
        ".vimrc" => (VIM, Some(0x43B05C)),
        "_vimrc" => (VIM, Some(0x43B05C)),
        ".gvimrc" => (VIM, Some(0x43B05C)),
        ".bashrc" => (SHELL, Some(0x5FD38D)),
        ".zshrc" => (SHELL, Some(0x5FD38D)),
        ".profile" => (SHELL, Some(0x5FD38D)),
        ".bash_profile" => (SHELL, Some(0x5FD38D)),
        ".zprofile" => (SHELL, Some(0x5FD38D)),
        ".bash_aliases" => (SHELL, Some(0x5FD38D)),
        "robots.txt" => (TEXT, Some(0x9AA5B1)),
        ".htaccess" => (INI, Some(0x9AA5B1)),
        ".npmrc" => (INI, Some(0x9AA5B1)),
        ".nvmrc" => (INI, Some(0x9AA5B1)),
        ".yarnrc" => (INI, Some(0x9AA5B1)),
        ".yarnrc.yml" => (INI, Some(0x9AA5B1)),
        ".editorconfig" => (INI, Some(0x9AA5B1)),
        ".gitconfig" => (INI, Some(0x9AA5B1)),
        "nginx.conf" => (INI, Some(0x9AA5B1)),
        ".env" => (ENV, Some(0xE8C547)),
        "package-lock.json" => (LOCK, Some(0x9AA5B1)),
        "npm-shrinkwrap.json" => (LOCK, Some(0x9AA5B1)),
        "yarn.lock" => (LOCK, Some(0x9AA5B1)),
        "pnpm-lock.yaml" => (LOCK, Some(0x9AA5B1)),
        "bun.lockb" => (LOCK, Some(0x9AA5B1)),
        "cargo.lock" => (LOCK, Some(0x9AA5B1)),
        "poetry.lock" => (LOCK, Some(0x9AA5B1)),
        "pipfile.lock" => (LOCK, Some(0x9AA5B1)),
        "composer.lock" => (LOCK, Some(0x9AA5B1)),
        "gemfile.lock" => (LOCK, Some(0x9AA5B1)),
        "go.sum" => (LOCK, Some(0x9AA5B1)),
        "flake.lock" => (LOCK, Some(0x9AA5B1)),
        "mix.lock" => (LOCK, Some(0x9AA5B1)),
        "pubspec.lock" => (LOCK, Some(0x9AA5B1)),
        "podfile.lock" => (LOCK, Some(0x9AA5B1)),
        "packages.lock.json" => (LOCK, Some(0x9AA5B1)),
        "uv.lock" => (LOCK, Some(0x9AA5B1)),
        "id_rsa" => (KEY, Some(0xD9A74A)),
        "id_ed25519" => (KEY, Some(0xD9A74A)),
        "id_ecdsa" => (KEY, Some(0xD9A74A)),
        "id_dsa" => (KEY, Some(0xD9A74A)),
        "authorized_keys" => (KEY, Some(0xD9A74A)),
        "known_hosts" => (KEY, Some(0xD9A74A)),
        ".gitignore" => (GIT, Some(0xF05033)),
        ".gitattributes" => (GIT, Some(0xF05033)),
        ".gitmodules" => (GIT, Some(0xF05033)),
        ".gitkeep" => (GIT, Some(0xF05033)),
        ".git-blame-ignore-revs" => (GIT, Some(0xF05033)),
        ".mailmap" => (GIT, Some(0xF05033)),
        "dockerfile" => (DOCKER, Some(0x2496ED)),
        "containerfile" => (DOCKER, Some(0x2496ED)),
        ".dockerignore" => (DOCKER, Some(0x2496ED)),
        "docker-compose.yml" => (DOCKER, Some(0x2496ED)),
        "docker-compose.yaml" => (DOCKER, Some(0x2496ED)),
        "compose.yml" => (DOCKER, Some(0x2496ED)),
        "compose.yaml" => (DOCKER, Some(0x2496ED)),
        "package.json" => (NPM, Some(0xE0464B)),
        ".npmignore" => (NPM, Some(0xE0464B)),
        "cargo.toml" => (CARGO, Some(0xDEA584)),
        "pyproject.toml" => (PYPACKAGE, Some(0x4F9FD8)),
        "pipfile" => (PYPACKAGE, Some(0x4F9FD8)),
        "requirements.txt" => (PYPACKAGE, Some(0x4F9FD8)),
        "setup.cfg" => (PYPACKAGE, Some(0x4F9FD8)),
        "environment.yml" => (PYPACKAGE, Some(0x4F9FD8)),
        "go.mod" => (GOMOD, Some(0x4FD1C5)),
        "go.work" => (GOMOD, Some(0x4FD1C5)),
        "gemfile" => (GEM, Some(0xE0464B)),
        "composer.json" => (COMPOSER, Some(0x8C8FD8)),
        "pom.xml" => (MAVEN, Some(0xE76F51)),
        "build.gradle" => (GRADLE, Some(0x5FB56B)),
        "build.gradle.kts" => (GRADLE, Some(0x5FB56B)),
        "settings.gradle" => (GRADLE, Some(0x5FB56B)),
        "settings.gradle.kts" => (GRADLE, Some(0x5FB56B)),
        "gradle.properties" => (GRADLE, Some(0x5FB56B)),
        "gradlew" => (GRADLE, Some(0x5FB56B)),
        "gradlew.bat" => (GRADLE, Some(0x5FB56B)),
        "pubspec.yaml" => (PUBSPEC, Some(0x2CB7F6)),
        "package.swift" => (SWIFTPKG, Some(0xF05138)),
        "podfile" => (SWIFTPKG, Some(0xF05138)),
        "mix.exs" => (MIX, Some(0x9B6CC9)),
        "stack.yaml" => (CABAL, Some(0x8F6CC4)),
        "makefile" => (BUILD, Some(0x9AA5B1)),
        "gnumakefile" => (BUILD, Some(0x9AA5B1)),
        "cmakelists.txt" => (BUILD, Some(0x9AA5B1)),
        "justfile" => (BUILD, Some(0x9AA5B1)),
        "meson.build" => (BUILD, Some(0x9AA5B1)),
        "build.ninja" => (BUILD, Some(0x9AA5B1)),
        "workspace" => (BUILD, Some(0x9AA5B1)),
        "build.bazel" => (BUILD, Some(0x9AA5B1)),
        "taskfile.yml" => (BUILD, Some(0x9AA5B1)),
        "procfile" => (BUILD, Some(0x9AA5B1)),
        "tsconfig.json" => (TSCONFIG, Some(0x3B8EEA)),
        "jsconfig.json" => (TSCONFIG, Some(0x3B8EEA)),
        ".prettierrc" => (TOOL_CONFIG, Some(0x9AA5B1)),
        ".eslintrc" => (TOOL_CONFIG, Some(0x9AA5B1)),
        ".stylelintrc" => (TOOL_CONFIG, Some(0x9AA5B1)),
        ".babelrc" => (TOOL_CONFIG, Some(0x9AA5B1)),
        ".browserslistrc" => (TOOL_CONFIG, Some(0x9AA5B1)),
        "vercel.json" => (TOOL_CONFIG, Some(0x9AA5B1)),
        "netlify.toml" => (TOOL_CONFIG, Some(0x9AA5B1)),
        "turbo.json" => (TOOL_CONFIG, Some(0x9AA5B1)),
        "nx.json" => (TOOL_CONFIG, Some(0x9AA5B1)),
        "lerna.json" => (TOOL_CONFIG, Some(0x9AA5B1)),
        ".gitlab-ci.yml" => (TOOL_CONFIG, Some(0x9AA5B1)),
        ".travis.yml" => (TOOL_CONFIG, Some(0x9AA5B1)),
        "appveyor.yml" => (TOOL_CONFIG, Some(0x9AA5B1)),
        "azure-pipelines.yml" => (TOOL_CONFIG, Some(0x9AA5B1)),
        "renovate.json" => (TOOL_CONFIG, Some(0x9AA5B1)),
        "readme" => (README, Some(0x8FB8FF)),
        "read.me" => (README, Some(0x8FB8FF)),
        "license" => (LICENSE, Some(0xD9A74A)),
        "licence" => (LICENSE, Some(0xD9A74A)),
        "copying" => (LICENSE, Some(0xD9A74A)),
        "unlicense" => (LICENSE, Some(0xD9A74A)),
        "changelog" => (CHANGELOG, Some(0x8FB8FF)),
        "history" => (CHANGELOG, Some(0x8FB8FF)),
        "changes" => (CHANGELOG, Some(0x8FB8FF)),
        "releases.md" => (CHANGELOG, Some(0x8FB8FF)),
        "contributing" => (CONTRIBUTING, Some(0x8FB8FF)),
        "code_of_conduct.md" => (CONTRIBUTING, Some(0x8FB8FF)),
        "security.md" => (CONTRIBUTING, Some(0x8FB8FF)),
        "authors" => (CONTRIBUTING, Some(0x8FB8FF)),
        "codeowners" => (CONTRIBUTING, Some(0x8FB8FF)),
        "conftest.py" => (TEST_PY, Some(0x4F9FD8)),
        _ => {
            if name.ends_with(".config.mjs") { return Some((TOOL_CONFIG, Some(0x9AA5B1))); }
            if name.ends_with(".config.cjs") { return Some((TOOL_CONFIG, Some(0x9AA5B1))); }
            if name.ends_with(".config.mts") { return Some((TOOL_CONFIG, Some(0x9AA5B1))); }
            if name.ends_with(".config.js") { return Some((TOOL_CONFIG, Some(0x9AA5B1))); }
            if name.ends_with(".config.ts") { return Some((TOOL_CONFIG, Some(0x9AA5B1))); }
            if name.ends_with(".test.mjs") { return Some((TEST_JS, Some(0xE8C547))); }
            if name.ends_with(".spec.mjs") { return Some((TEST_JS, Some(0xE8C547))); }
            if name.ends_with(".test.cjs") { return Some((TEST_JS, Some(0xE8C547))); }
            if name.ends_with(".spec.cjs") { return Some((TEST_JS, Some(0xE8C547))); }
            if name.ends_with(".test.jsx") { return Some((TEST_JS, Some(0xE8C547))); }
            if name.ends_with(".spec.jsx") { return Some((TEST_JS, Some(0xE8C547))); }
            if name.ends_with(".test.tsx") { return Some((TEST_TS, Some(0x3B8EEA))); }
            if name.ends_with(".spec.tsx") { return Some((TEST_TS, Some(0x3B8EEA))); }
            if name.ends_with(".test.mts") { return Some((TEST_TS, Some(0x3B8EEA))); }
            if name.ends_with(".spec.mts") { return Some((TEST_TS, Some(0x3B8EEA))); }
            if name.ends_with(".test.js") { return Some((TEST_JS, Some(0xE8C547))); }
            if name.ends_with(".spec.js") { return Some((TEST_JS, Some(0xE8C547))); }
            if name.ends_with(".test.ts") { return Some((TEST_TS, Some(0x3B8EEA))); }
            if name.ends_with(".spec.ts") { return Some((TEST_TS, Some(0x3B8EEA))); }
            if name.ends_with("_test.py") { return Some((TEST_PY, Some(0x4F9FD8))); }
            if name.ends_with("_test.go") { return Some((TEST_GO, Some(0x4FD1C5))); }
            if name.ends_with("_spec.rb") { return Some((TEST_RB, Some(0xE0464B))); }
            if name.ends_with("_test.rb") { return Some((TEST_RB, Some(0xE0464B))); }
            if name.ends_with("_test.rs") { return Some((TEST_RS, Some(0xDEA584))); }
            if name.ends_with(".d.mts") { return Some((TS_DECL, Some(0x7FA6C9))); }
            if name.ends_with(".d.cts") { return Some((TS_DECL, Some(0x7FA6C9))); }
            if name.ends_with(".d.ts") { return Some((TS_DECL, Some(0x7FA6C9))); }
            if name.ends_with(".env") { return Some((ENV, Some(0xE8C547))); }
            if name.starts_with("docker-compose.") && name.ends_with(".yml") && name.len() >= 19 { return Some((DOCKER, Some(0x2496ED))); }
            if name.starts_with("docker-compose.") && name.ends_with(".yaml") && name.len() >= 20 { return Some((DOCKER, Some(0x2496ED))); }
            if name.starts_with("requirements-") && name.ends_with(".txt") && name.len() >= 17 { return Some((PYPACKAGE, Some(0x4F9FD8))); }
            if name.starts_with("requirements_") && name.ends_with(".txt") && name.len() >= 17 { return Some((PYPACKAGE, Some(0x4F9FD8))); }
            if name.starts_with("contributing.") { return Some((CONTRIBUTING, Some(0x8FB8FF))); }
            if name.starts_with(".prettierrc.") { return Some((TOOL_CONFIG, Some(0x9AA5B1))); }
            if name.starts_with("dockerfile.") { return Some((DOCKER, Some(0x2496ED))); }
            if name.starts_with(".eslintrc.") { return Some((TOOL_CONFIG, Some(0x9AA5B1))); }
            if name.starts_with("changelog.") { return Some((CHANGELOG, Some(0x8FB8FF))); }
            if name.starts_with("tsconfig.") && name.ends_with(".json") && name.len() >= 14 { return Some((TSCONFIG, Some(0x3B8EEA))); }
            if name.starts_with("license.") { return Some((LICENSE, Some(0xD9A74A))); }
            if name.starts_with("licence.") { return Some((LICENSE, Some(0xD9A74A))); }
            if name.starts_with("copying.") { return Some((LICENSE, Some(0xD9A74A))); }
            if name.starts_with("history.") { return Some((CHANGELOG, Some(0x8FB8FF))); }
            if name.starts_with("changes.") { return Some((CHANGELOG, Some(0x8FB8FF))); }
            if name.starts_with("authors.") { return Some((CONTRIBUTING, Some(0x8FB8FF))); }
            if name.starts_with("readme.") { return Some((README, Some(0x8FB8FF))); }
            if name.starts_with(".env.") { return Some((ENV, Some(0xE8C547))); }
            if name.starts_with("test_") && name.ends_with(".py") && name.len() >= 8 { return Some((TEST_PY, Some(0x4F9FD8))); }
            return None;
        }
    };
    Some(hit)
}

fn by_ext(ext: &str) -> Option<(FluxIcon, Option<u32>)> {
    Some(match ext {
        "py" | "pyw" | "pyi" | "pyx" | "pxd" | "pyz" | "rpy" => (PYTHON, Some(0x4F9FD8)),
        "js" | "mjs" | "cjs" | "es6" => (JAVASCRIPT, Some(0xE8C547)),
        "ts" | "mts" | "cts" => (TYPESCRIPT, Some(0x3B8EEA)),
        "jsx" => (JSX, Some(0x61DAFB)),
        "tsx" => (TSX, Some(0x3B8EEA)),
        "rs" => (RUST, Some(0xDEA584)),
        "go" => (GO, Some(0x4FD1C5)),
        "c" | "i" => (C, Some(0x6C9BD2)),
        "h" => (C_HEADER, Some(0xA48ADB)),
        "cpp" | "cc" | "cxx" | "c++" | "ixx" | "cppm" | "tpp" | "ipp" | "inl" => (CPP, Some(0x5C8DD6)),
        "hpp" | "hh" | "hxx" | "h++" => (CPP_HEADER, Some(0x5C8DD6)),
        "cs" | "csx" => (CSHARP, Some(0x9B6CD8)),
        "fs" | "fsi" | "fsx" | "fsscript" => (FSHARP, Some(0x3A9BD5)),
        "vb" | "vbs" | "vba" | "bas" => (VB, Some(0x6F86D9)),
        "java" | "jav" => (JAVA, Some(0xE76F51)),
        "class" => (CLASS, Some(0xE76F51)),
        "kt" | "kts" => (KOTLIN, Some(0xA97BFF)),
        "scala" | "sc" | "sbt" => (SCALA, Some(0xDE3423)),
        "groovy" | "gvy" | "gy" | "gsh" => (GROOVY, Some(0x4298B8)),
        "clj" | "cljs" | "cljc" | "edn" => (CLOJURE, Some(0x63B132)),
        "swift" => (SWIFT, Some(0xF05138)),
        "m" | "mm" => (OBJC, Some(0x6C9BD2)),
        "dart" => (DART, Some(0x2CB7F6)),
        "rb" | "rbw" | "rake" | "gemspec" | "ru" | "erb" => (RUBY, Some(0xE0464B)),
        "php" | "phtml" | "php3" | "php4" | "php5" | "phps" | "phpt" => (PHP, Some(0x8C8FD8)),
        "pl" | "pm" | "pod" | "t" | "psgi" => (PERL, Some(0x6C8ECF)),
        "raku" | "rakumod" | "p6" | "pm6" => (RAKU, Some(0xC7B4E8)),
        "lua" | "luau" | "rockspec" => (LUA, Some(0x5A7FE8)),
        "r" | "rmd" | "rdata" | "rds" => (R, Some(0x3F7FCF)),
        "jl" => (JULIA, Some(0xA270BA)),
        "hs" | "lhs" | "hsc" => (HASKELL, Some(0x8F6CC4)),
        "ml" | "mli" | "mll" | "mly" => (OCAML, Some(0xEE7A00)),
        "re" | "rei" | "res" | "resi" => (REASON, Some(0xDD4B39)),
        "purs" => (PURESCRIPT, Some(0x9CA3AF)),
        "elm" => (ELM, Some(0x60B5CC)),
        "erl" | "hrl" => (ERLANG, Some(0xB83998)),
        "ex" | "exs" | "eex" | "heex" | "leex" => (ELIXIR, Some(0x9B6CC9)),
        "gleam" => (GLEAM, Some(0xFFAFF3)),
        "zig" | "zon" => (ZIG, Some(0xF7A41D)),
        "nim" | "nims" | "nimble" => (NIM, Some(0xEBC64F)),
        "cr" => (CRYSTAL, Some(0xC9CED6)),
        "d" | "di" => (D, Some(0xC74B3F)),
        "v" | "vsh" => (V, Some(0x5D87BF)),
        "odin" => (ODIN, Some(0x3882D2)),
        "hx" | "hxml" => (HAXE, Some(0xEA8220)),
        "mojo" => (MOJO, Some(0xFF6A33)),
        "f" | "f77" | "f90" | "f95" | "f03" | "f08" | "for" | "ftn" => (FORTRAN, Some(0x7C6FD1)),
        "cob" | "cbl" | "cpy" => (COBOL, Some(0x5E81AC)),
        "pas" | "pp" | "dpr" | "dpk" | "lpr" => (PASCAL, Some(0xD9A74A)),
        "adb" | "ads" | "ada" | "gpr" => (ADA, Some(0x5A9BD8)),
        "lisp" | "lsp" | "cl" | "fasl" | "el" | "scm" | "ss" | "sld" | "rkt" | "rktl" => (LISP, Some(0xC792EA)),
        "pro" => (PROLOG, Some(0xE8704A)),
        "asm" | "s" | "nasm" | "masm" | "a51" | "inc" => (ASM, Some(0x9AA5B1)),
        "wasm" | "wat" | "wast" => (WASM, Some(0x7A6BF0)),
        "sol" => (SOLIDITY, Some(0x8A9AB8)),
        "cu" | "cuh" => (CUDA, Some(0x76B900)),
        "glsl" | "vert" | "frag" | "geom" | "comp" | "tesc" | "tese" | "hlsl" | "wgsl" | "shader" | "cginc" | "fx" | "metal" => (SHADER, Some(0xE58BB8)),
        "sv" | "svh" | "vh" | "vhd" | "vhdl" => (VERILOG, Some(0x6CB38A)),
        "ino" | "pde" => (ARDUINO, Some(0x00A3A8)),
        "gd" | "gdshader" | "tscn" | "tres" | "godot" => (GDSCRIPT, Some(0x5A9BD8)),
        "coffee" | "cson" | "iced" => (COFFEE, Some(0xC8A97E)),
        "apex" | "trigger" => (APEX, Some(0x4AA8E0)),
        "qs" => (QSHARP, Some(0x9B6CD8)),
        "wl" | "wls" | "nb" => (WOLFRAM, Some(0xDD4444)),
        "mat" | "mlx" => (MATLAB, Some(0xE8853A)),
        "tcl" | "tk" => (TCL, Some(0x8FB8DE)),
        "vim" | "vimrc" => (VIM, Some(0x43B05C)),
        "applescript" | "scpt" => (APPLESCRIPT, Some(0xA8B1BC)),
        "ahk" => (AUTOHOTKEY, Some(0x6DB33F)),
        "sh" | "bash" | "zsh" | "fish" | "ksh" | "csh" | "tcsh" | "ash" | "dash" | "command" | "awk" => (SHELL, Some(0x5FD38D)),
        "ps1" | "psm1" | "psd1" | "ps1xml" | "pssc" => (POWERSHELL, Some(0x4C8FD6)),
        "bat" | "cmd" | "btm" => (BATCH, Some(0xA8B1BC)),
        "html" | "htm" | "xhtml" | "shtml" => (HTML, Some(0xE8704A)),
        "css" | "pcss" | "postcss" => (CSS, Some(0x5AA7E8)),
        "scss" | "sass" => (SCSS, Some(0xDB6A9F)),
        "less" => (LESS, Some(0x5B85C7)),
        "styl" | "stylus" => (STYLUS, Some(0xB6D96C)),
        "vue" => (VUE, Some(0x41B883)),
        "svelte" => (SVELTE, Some(0xFF5A1F)),
        "astro" => (ASTRO, Some(0xFF7A3D)),
        "hbs" | "handlebars" | "mustache" => (HANDLEBARS, Some(0xF4A261)),
        "pug" | "jade" => (PUG, Some(0xC9A27E)),
        "haml" | "slim" => (HAML, Some(0xECD9A0)),
        "ejs" => (EJS, Some(0xB4CA65)),
        "liquid" => (LIQUID, Some(0x76C7C0)),
        "twig" => (TWIG, Some(0x9ACD5E)),
        "cshtml" | "razor" | "vbhtml" => (RAZOR, Some(0x9B6CD8)),
        "j2" | "jinja" | "jinja2" | "njk" | "nunjucks" => (JINJA, Some(0xC9A27E)),
        "graphql" | "gql" | "graphqls" => (GRAPHQL, Some(0xE535AB)),
        "md" | "markdown" | "mdown" | "mkd" | "mdx" => (MARKDOWN, Some(0x8FB8FF)),
        "rst" | "rest" => (RST, Some(0x8FB8FF)),
        "adoc" | "asciidoc" => (ASCIIDOC, Some(0xE76F51)),
        "tex" | "ltx" | "sty" | "cls" | "dtx" | "ins" | "bib" | "bst" => (LATEX, Some(0x67A6A2)),
        "typ" => (TYPST, Some(0x3AC7D4)),
        "org" => (ORG, Some(0x77AA99)),
        "txt" | "text" | "rtf" | "nfo" | "me" | "1st" => (TEXT, Some(0x9AA5B1)),
        "flux" | "flx" => (FLUX, Some(0xC792EA)),
        "pdf" => (PDF, Some(0xE5534B)),
        "doc" | "docx" | "odt" | "dot" | "dotx" | "pages" | "wpd" => (DOC, Some(0x4F8FE0)),
        "xls" | "xlsx" | "xlsm" | "ods" | "numbers" => (SHEET, Some(0x3FB27F)),
        "ppt" | "pptx" | "odp" => (SLIDES, Some(0xE8704A)),
        "epub" | "mobi" | "azw" | "azw3" | "fb2" => (EBOOK, Some(0xD9A74A)),
        "ipynb" => (NOTEBOOK, Some(0xF2884B)),
        "diff" | "patch" | "rej" => (DIFF, Some(0x5FD38D)),
        "log" | "out" => (LOG, Some(0x9AA5B1)),
        "json" | "jsonc" | "json5" | "jsonl" | "ndjson" | "geojson" | "topojson" | "har" | "webmanifest" | "map" | "babelrc" | "eslintrc" | "prettierrc" | "stylelintrc" => (JSON, Some(0xD9A74A)),
        "yaml" | "yml" => (YAML, Some(0xE0646E)),
        "toml" => (TOML, Some(0xC88B5A)),
        "ini" | "cfg" | "conf" | "config" | "properties" | "prefs" | "reg" | "inf" | "desktop" | "service" => (INI, Some(0x9AA5B1)),
        "xml" | "xsd" | "xsl" | "xslt" | "xaml" | "plist" | "resx" | "svgz" | "rss" | "atom" | "wsdl" | "xib" | "storyboard" | "manifest" => (XML, Some(0xE8A04A)),
        "csv" | "tsv" | "psv" | "tab" => (CSV, Some(0x5FD38D)),
        "parquet" | "avro" | "orc" | "arrow" | "feather" => (COLUMNAR, Some(0x5AA7E8)),
        "proto" => (PROTO, Some(0x6C9BD2)),
        "tf" | "tfvars" | "tfstate" | "hcl" | "nomad" => (TERRAFORM, Some(0x8C6CE8)),
        "nix" => (NIX, Some(0x7EB2DD)),
        "bicep" => (BICEP, Some(0x4AA8E0)),
        "env" => (ENV, Some(0xE8C547)),
        "sql" | "psql" | "pgsql" | "plsql" | "mysql" | "tsql" | "ddl" | "dml" | "dbml" | "cql" | "cypher" | "hql" | "prql" => (SQL, Some(0xE8A04A)),
        "sqlite" | "sqlite3" | "db" | "db3" | "s3db" | "sl3" | "duckdb" => (SQLITE, Some(0x4F9FD8)),
        "mdb" | "accdb" | "frm" | "ibd" | "myd" | "dbf" | "realm" | "rdb" | "ldb" | "kdbx" | "dump" => (DB_FILE, Some(0xA48ADB)),
        "prisma" => (PRISMA, Some(0x8C98E8)),
        "bson" => (MONGO, Some(0x5FB56B)),
        "png" | "jpg" | "jpeg" | "jfif" | "gif" | "webp" | "bmp" | "ico" | "icns" | "avif" | "tif" | "tiff" | "heic" | "heif" | "tga" | "dds" | "raw" | "cr2" | "nef" | "psd" | "xcf" | "kra" => (IMAGE, Some(0xC792EA)),
        "svg" | "ai" | "eps" => (SVG, Some(0xF2B441)),
        "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi" | "wmv" | "flv" | "mpg" | "mpeg" | "3gp" | "ogv" => (VIDEO, Some(0xE5534B)),
        "mp3" | "wav" | "ogg" | "oga" | "flac" | "m4a" | "aac" | "opus" | "wma" | "aiff" | "mid" | "midi" => (AUDIO, Some(0x4FD1C5)),
        "ttf" | "otf" | "woff" | "woff2" | "eot" | "fon" | "fnt" | "pfb" => (FONT, Some(0x9AA5B1)),
        "fbx" | "gltf" | "glb" | "stl" | "blend" | "dae" | "3ds" | "ply" | "usd" | "usda" | "usdz" | "abc" => (MODEL3D, Some(0x6CB3D8)),
        "exe" | "dll" | "so" | "dylib" | "o" | "obj" | "a" | "lib" | "bin" | "dat" | "pdb" | "elf" | "ko" | "sys" | "msi" => (BINARY, Some(0x9AA5B1)),
        "zip" | "7z" | "rar" | "gz" | "tgz" | "tar" | "bz2" | "xz" | "zst" | "lz" | "lzma" | "cab" | "iso" | "dmg" | "img" | "whl" | "jar" | "war" | "ear" | "apk" | "aab" | "ipa" | "deb" | "rpm" | "nupkg" | "crate" | "gem" | "vsix" | "snap" | "appimage" => (ARCHIVE, Some(0xC88B5A)),
        "lock" | "lockb" => (LOCK, Some(0x9AA5B1)),
        "pem" | "crt" | "cer" | "der" | "csr" | "p7b" | "p7c" => (CERT, Some(0x5FD38D)),
        "key" | "pub" | "p12" | "pfx" | "jks" | "keystore" | "gpg" | "asc" | "sig" => (KEY, Some(0xD9A74A)),
        "dockerfile" => (DOCKER, Some(0x2496ED)),
        "gradle" => (GRADLE, Some(0x5FB56B)),
        "csproj" | "fsproj" | "vbproj" | "vcxproj" | "sln" | "props" | "targets" | "nuspec" => (DOTNET, Some(0x9B6CD8)),
        "cabal" => (CABAL, Some(0x8F6CC4)),
        "mk" | "mak" | "cmake" | "ninja" | "bazel" | "bzl" => (BUILD, Some(0x9AA5B1)),
        _ => return None,
    })
}

pub fn for_name(name: &str) -> Option<(FluxIcon, Option<u32>)> {
    let lower = name.to_ascii_lowercase();
    if let Some(hit) = by_name(&lower) {
        return Some(hit);
    }
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    by_ext(ext)
}

#[allow(dead_code)]
pub const KINDS: usize = 166;
