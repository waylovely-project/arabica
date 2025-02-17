use jreflection::{io_data_err, io_data_error};

mod android;
mod config;
mod emit_rust;
mod identifiers;
mod run;
mod util;

fn main() {
    entry::main();
}

mod entry {
    use crate::run::{run, RunResult};
    use crate::*;

    use bugsalot::debugger;

    use clap::load_yaml;

    use std::fs::File;
    use std::io::{self, BufRead, BufReader, BufWriter, Write};
    use std::path::*;
    use std::process::exit;

    pub fn main() {
        let yaml = load_yaml!("../cli.yml");
        let matches = clap::App::from_yaml(yaml).get_matches();

        let _help = matches.is_present("help");
        let directory: &Path = Path::new(matches.value_of("directory").unwrap_or("."));
        let _verbose = matches.is_present("verbose");
        let android_api_levels  = matches.value_of("android-api-levels").map(|api| api.parse::<android::ApiLevelRange>().expect("--android-api-levels must take the form of a single version like '8', or a range like '8-27'"));

        if let Some(api_levels) = android_api_levels.as_ref() {
            if api_levels.start() < 7 {
                eprintln!("\
                    WARNING:  Untested api level {} (<7).\n\
                    If you've found where I can grab such an early version of the Android SDK/APIs,\n\
                    please comment on / reopen https://github.com/MaulingMonkey/jni-bindgen/issues/10 !",
                    api_levels.start()
                );
            }
        }

        let subcommand = matches.subcommand_name().unwrap_or("generate");

        match subcommand {
            "generate" => {
                let mut config_file = config::toml::File::from_directory(directory).unwrap();

                let result = if let Some(api_levels) = android_api_levels.as_ref() {
                    let mut result = None;
                    for api_level in api_levels.iter() {
                        let sdk_android_jar = if std::env::var_os("ANDROID_HOME").is_some() {
                            format!("%ANDROID_HOME%/platforms/android-{}/android.jar", api_level)
                        } else if cfg!(windows) {
                            format!(
                                "%LOCALAPPDATA%/Android/Sdk/platforms/android-{}/android.jar",
                                api_level
                            )
                        } else {
                            panic!("ANDROID_HOME not defined and not automatically inferrable on this platform");
                        };

                        config_file.file.input.files.clear();
                        config_file
                            .file
                            .input
                            .files
                            .push(PathBuf::from(sdk_android_jar));
                        config_file.file.output.path =
                            PathBuf::from(format!("src/generated/api-level-{}.rs", api_level));
                        result = Some(run(config_file.clone()).unwrap())
                    }
                    result.unwrap()
                } else {
                    run(config_file).unwrap()
                };

                if let Err(e) = generate_toml(directory, android_api_levels.as_ref(), &result) {
                    eprintln!("ERROR:  Failed to regenerate Cargo.toml:\n    {:?}", e);
                    exit(1);
                }
            }
            "verify" => {
                eprintln!("verify not yet implemented");
                debugger::break_if_attached();
                exit(1);
            }
            unknown => {
                eprintln!("Unexpected subcommand: {}", unknown);
                debugger::break_if_attached();
                exit(1);
            }
        }
    }

    fn generate_toml(
        directory: &Path,
        api_levels: Option<&android::ApiLevelRange>,
        result: &RunResult,
    ) -> io::Result<()> {
        // XXX: Check that Cargo.toml is marked as generated

        let template = BufReader::new(File::open(directory.join("Cargo.toml.template"))?);
        let mut out = BufWriter::new(File::create(directory.join("Cargo.toml"))?);

        writeln!(out, "# WARNING:  This file was autogenerated by jni-bindgen.  Any changes to this file may be lost!!!")?;
        writeln!(out)?;

        for line in template.lines() {
            let line = line?;
            let line = line.trim_end_matches(|ch| ch == '\n' || ch == '\r');
            match line {
                "# PLACEHOLDER:FEATURES:api-level-NN" => {
                    if let Some(api_levels) = api_levels {
                        writeln!(out, "{}:BEGIN", line)?;
                        for api_level in api_levels.iter() {
                            write!(out, "api-level-{} = [", api_level)?;
                            if api_level > api_levels.start() {
                                write!(out, "\"api-level-{}\"", api_level - 1)?;
                            }
                            writeln!(out, "]")?;
                        }
                        writeln!(out, "{}:END", line)?;
                    } else {
                        writeln!(out, "{}:N/A", line)?;
                    }
                }
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
                }
                "# PLACEHOLDER:FEATURES:docs.rs" => {
                    writeln!(out, "{}:BEGIN", line)?;
                    if let Some(api_levels) = api_levels {
                        writeln!(
                            out,
                            "features = [\"all\", \"api-level-{}\", \"force-define\"]",
                            api_levels.end()
                        )?;
                    } else {
                        writeln!(out, "features = [\"all\", \"force-define\"]")?;
                    }
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
