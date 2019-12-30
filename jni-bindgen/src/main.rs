#[macro_use] mod java;

/// Configuration formats for invoking jni_bindgen
mod config { // Part of the actual official API of this crate.
    #[allow(unused_imports)] use super::*;

    pub mod runtime;
    pub mod toml;
}

/// Rust generation logic
mod emit_rust {
    #[allow(unused_imports)] use super::*;

    mod context;
    mod fields;
    mod known_docs_url;
    mod methods;
    mod modules;
    mod preamble;
    mod structs;

    pub use context::Context;
    use fields::*;
    use known_docs_url::*;
    use methods::*;
    use modules::*;
    use preamble::*;
    use structs::*;
}

/// JNI and Rust identifier parsing and categorizing utilities
mod identifiers {
    #[allow(unused_imports)] use super::*;
    use std::iter::*;

    mod field_mangling_style;
    mod method_mangling_style;
    mod rust_identifier;

    pub use field_mangling_style::*;
    pub use method_mangling_style::*;
    pub use rust_identifier::*;
}

/// Core generation logic
mod run {
    #[allow(unused_imports)] use super::*;

    mod run;

    pub use run::run;
    pub use run::RunResult;
}

mod util {
    #[allow(unused_imports)] use super::*;

    mod dedupe_file_set;
    mod difference;
    mod generated_file;
    mod progress;

    pub use dedupe_file_set::{ConcurrentDedupeFileSet, DedupeFileSet};
    pub use difference::Difference;
    pub use generated_file::write_generated;
    pub use progress::Progress;
}


use identifiers::*;
use run::run;
use run::RunResult;



fn main() {
    entry::main();
}

mod entry {
    use crate::*;
    use bugsalot::debugger;

    use clap::load_yaml;

    use std::fs::{File};
    use std::io::{self, BufRead, BufReader, BufWriter, Write};
    
    use std::ops::*;
    use std::path::*;
    use std::process::exit;

    pub fn main() {
        std::panic::set_hook(Box::new(|panic|{ bugsalot::bug!("{:?}", panic); }));
        // jni_bindgen::run(jni_bindgen::config::toml::File::from_current_directory().unwrap()).unwrap(); // Old legacy behavior

        let yaml = load_yaml!("../cli.yml");
        let matches = clap::App::from_yaml(yaml).get_matches();

        let _help               = matches.is_present("help");
        let directory : &Path   = Path::new(matches.value_of("directory").unwrap_or("../jni-android-sys"));
        let _verbose            = matches.is_present("verbose");
        let min_api_level : i32 = matches.value_of("min-api-level").unwrap_or( "7").parse().expect("--min-api-level must specify an integer version");
        let max_api_level : i32 = matches.value_of("max-api-level").unwrap_or("28").parse().expect("--max-api-level must specify an integer version");

        if min_api_level < 7 {
            eprintln!("\
                WARNING:  Untested api level {} (<7).\n\
                If you've found where I can grab such an early version of the Android SDK/APIs,\n\
                please comment on / reopen https://github.com/MaulingMonkey/jni-bindgen/issues/10 !",
                min_api_level
            );
        }

        let api_levels = min_api_level..=max_api_level;

        let subcommand = matches.subcommand_name().unwrap_or("generate");

        match subcommand {
            "generate" => {
                let mut config_file = config::toml::File::from_directory(directory).unwrap();

                let mut result = None;
                for api_level in api_levels.clone() {
                    let sdk_android_jar = if std::env::var_os("ANDROID_HOME").is_some() {
                        format!("%ANDROID_HOME%/platforms/android-{}/android.jar", api_level)
                    } else if cfg!(windows) {
                        format!("%LOCALAPPDATA%/Android/Sdk/platforms/android-{}/android.jar", api_level)
                    } else {
                        panic!("ANDROID_HOME not defined and not automatically inferrable on this platform");
                    };

                    config_file.file.input.files.clear();
                    config_file.file.input.files.push(PathBuf::from(sdk_android_jar));
                    config_file.file.output.path = PathBuf::from(format!("src/generated/api-level-{}.rs", api_level));
                    result = run(config_file.clone()).ok();
                }
                let result = result.unwrap();

                if let Err(e) = generate_toml(directory, api_levels.clone(), &result) {
                    eprintln!("ERROR:  Failed to regenerate Cargo.toml:\n    {:?}", e);
                    exit(1);
                }
            },
            "verify" => {
                eprintln!("verify not yet implemented");
                debugger::break_if_attached();
                exit(1);
            },
            unknown => {
                eprintln!("Unexpected subcommand: {}", unknown);
                debugger::break_if_attached();
                exit(1);
            },
        }
    }

    fn generate_toml(directory: &Path, api_levels: RangeInclusive<i32>, result: &RunResult) -> io::Result<()> {
        // XXX: Check that Cargo.toml is marked as generated

        let template    = BufReader::new(File::open(directory.join("Cargo.toml.template"))?);
        let mut out     = BufWriter::new(File::create(directory.join("Cargo.toml"))?);

        writeln!(out, "# WARNING:  This file was autogenerated by jni-bindgen.  Any changes to this file may be lost!!!")?;
        writeln!(out, "")?;

        for line in template.lines() {
            let line = line?;
            let line = line.trim_end_matches(|ch| ch == '\n' || ch == '\r');
            match line {
                "# PLACEHOLDER:FEATURES:api-level-NN" => {
                    writeln!(out, "{}:BEGIN", line)?;
                    for api_level in api_levels.clone() {
                        write!(out, "api-level-{} = [", api_level)?;
                        if api_level > *api_levels.start() {
                            write!(out, "\"api-level-{}\"", api_level-1)?;
                        }
                        writeln!(out, "]")?;
                    }
                    writeln!(out, "{}:END", line)?;
                },
                "# PLACEHOLDER:FEATURES:sharded-api" => {
                    writeln!(out, "{}:BEGIN", line)?;
                    for (feature, dependencies) in result.features.iter() {
                        write!(out, "{:?} = [", feature)?;
                        for (idx, dependency) in dependencies.iter().enumerate() {
                            if idx != 0 {
                                write!(out, ", ")?;
                            }
                            write!(out, "{:?}", dependency)?;
                        }
                        writeln!(out, "]")?;
                    }

                    // Wildcard feature "*".  While it's tempting to make this depend on all other features, this
                    // causes problems on windows where we run into command line length limits invoking rustc.
                    writeln!(out, "\"all\" = []")?;
                    writeln!(out, "{}:END", line)?;
                },
                "# PLACEHOLDER:FEATURES:docs.rs" => {
                    writeln!(out, "{}:BEGIN", line)?;
                    writeln!(out, "features = [\"all\", \"api-level-{}\", \"force-define\"]", api_levels.end())?;
                    writeln!(out, "{}:END", line)?;
                }
                line => {
                    if line.starts_with("# PLACEHOLDER:") {
                        eprintln!("WARNING:  Unexpected Cargo.toml placeholder:\n    {}", line);
                    }
                    writeln!(out, "{}", line)?;
                }
            }
        }

        Ok(())
    }
}
