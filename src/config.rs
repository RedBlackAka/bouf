use std::path::{Path, PathBuf};
use std::{env, fs};

use clap::Parser;
use serde::Deserialize;
use toml;

use crate::utils::errors::SomeError;
use crate::utils::misc;

#[derive(Parser, Debug)]
#[clap(about, long_about = None)]
pub struct MainArgs {
    // Required
    #[clap(short, long, value_parser, value_name = "Config file")]
    pub config: PathBuf,
    /// OBS main version
    #[clap(long, value_parser, value_name = "Major.Minor.Patch[-(rc|beta)Num]")]
    pub version: String,

    // Optional version suffix
    #[clap(long, value_parser, value_name = "Beta number")]
    pub beta: Option<u8>,
    #[clap(long, value_parser, value_name = "RC number")]
    pub rc: Option<u8>,
    #[clap(long, value_parser, value_name = "Beta branch")]
    pub branch: Option<String>,

    // Optional overrides
    #[clap(long, value_parser, value_name = "new build")]
    pub new: Option<PathBuf>,
    #[clap(long, value_parser, value_name = "old builds")]
    pub old: Option<PathBuf>,
    #[clap(long, value_parser, value_name = "output dir")]
    pub out: Option<PathBuf>,
    /// File containing release notes
    #[clap(long, value_parser, value_name = "file.rtf")]
    pub note_file: Option<PathBuf>,
    /// Falls back to "UPDATER_PRIVATE_KEY" env var
    #[clap(short, long, value_parser, value_name = "file.pem")]
    pub private_key: Option<PathBuf>,

    // Optional flags
    /// Skip creating NSIS installer
    #[clap(long, value_parser, default_value_t = false)]
    pub skip_installer: bool,
    /// Skip creating delta patches
    #[clap(long, value_parser, default_value_t = false)]
    pub skip_patches: bool,
    /// Skip codesigning
    #[clap(long, value_parser, default_value_t = false)]
    pub skip_codesigning: bool,
    /// Skip signing manifest
    #[clap(long, value_parser, default_value_t = false)]
    pub skip_manifest_signing: bool,
    /// Clear existing output directory
    #[clap(short, long, value_parser, default_value_t = false)]
    pub clear_output: bool,
}

fn get_default_branch() -> String {
    String::from("stable")
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub env: EnvOptions,
    pub prepare: PreparationOptions,
    pub generate: GenerationOptions,
    pub package: PackageOptions,
    pub post: PostOptions,
    pub obs_version: ObsVersion,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ObsVersion {
    pub version_str: String,
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    pub beta: u8,
    pub rc: u8,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct EnvOptions {
    #[serde(default = "get_default_branch")]
    pub branch: String,
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub previous_dir: PathBuf,
    // Tool paths
    pub sevenzip_path: PathBuf,
    pub makensis_path: PathBuf,
    pub pandoc_path: PathBuf,
    pub pdbcopy_path: PathBuf,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PreparationOptions {
    pub copy: CopyOptions,
    pub codesign: CodesignOptions,
    pub strip_pdbs: StripPDBOptions,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CopyOptions {
    pub excludes: Vec<String>,
    pub overrides: Vec<(String, String)>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CodesignOptions {
    pub skip_sign: bool,
    pub sign_name: String,
    pub sign_digest: String,
    pub sign_ts_serv: String,
    pub sign_exts: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct StripPDBOptions {
    pub exclude: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct GenerationOptions {
    // patch_type: String,
    pub removed_files: Vec<String>,
    pub packages: Vec<ManifestPackageOptions>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ManifestPackageOptions {
    pub name: String,
    pub include_files: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PackageOptions {
    pub installer: InstallerOptions,
    pub zip: ZipOptions,
    pub updater: UpdaterOptions,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct InstallerOptions {
    pub nsis_script: PathBuf,
    pub name: String,
    pub skip_sign: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ZipOptions {
    pub name: String,
    pub pdb_name: String,
    pub skip_for_prerelease: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct UpdaterOptions {
    pub skip_sign: bool,
    pub notes_files: PathBuf,
    pub updater_path: PathBuf,
    pub private_key: PathBuf,
    pub vc_redist_path: PathBuf,
    pub skip_for_prerelease: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PostOptions {
    pub move_to_old: bool,
}

impl Config {
    pub fn set_version(&mut self, version_string: &String, beta_num: u8, rc_num: u8) {
        let ver_parsed = misc::parse_version(version_string);
        self.obs_version.version_str = version_string.split("-").next().unwrap().to_string();
        self.obs_version.version_major = ver_parsed.0;
        self.obs_version.version_minor = ver_parsed.1;
        self.obs_version.version_patch = ver_parsed.2;
        self.obs_version.beta = if beta_num > 0 { beta_num } else { ver_parsed.3 };
        self.obs_version.rc = if rc_num > 0 { rc_num } else { ver_parsed.4 };
    }

    pub fn set_dirs(&mut self, input: Option<PathBuf>, output: Option<PathBuf>, previous: Option<PathBuf>) {
        if let Some(input) = input {
            self.env.input_dir = input;
        }
        if let Some(output) = output {
            self.env.output_dir = output;
        }
        if let Some(previous) = previous {
            self.env.previous_dir = previous;
        }
    }

    pub fn apply_args(&mut self, args: &MainArgs) {
        self.set_version(
            &args.version,
            args.beta.unwrap_or_default(),
            args.rc.unwrap_or_default(),
        );
        if let Some(input) = &args.new {
            self.env.input_dir = input.clone();
        }
        if let Some(output) = &args.out {
            self.env.output_dir = output.clone();
        }
        if let Some(previous) = &args.old {
            self.env.previous_dir = previous.clone();
        }
        if let Some(branch) = &args.branch {
            self.env.branch = branch.to_owned();
        }
        if let Some(beta) = &args.beta {
            self.obs_version.beta = *beta;
        }
        if let Some(rc) = &args.rc {
            self.obs_version.rc = *rc;
        }

        self.prepare.codesign.skip_sign = args.skip_codesigning;
        self.package.installer.skip_sign = args.skip_codesigning;
        self.package.updater.skip_sign = !args.skip_manifest_signing;

        // Todo remaining args
    }

    pub fn validate(&mut self, check_binaries: bool, check_paths: bool) -> Result<(), SomeError> {
        // Check file paths (for binaries, also check if they are in %PATH%)
        if check_binaries {
            misc::check_binary_path(&mut self.env.pdbcopy_path)?;
            misc::check_binary_path(&mut self.env.makensis_path)?;
            misc::check_binary_path(&mut self.env.sevenzip_path)?;
            misc::check_binary_path(&mut self.env.pandoc_path)?;
        }
        // Check if private key is set correctly (if signing is enabled)
        if !self.package.updater.skip_sign {
            if env::var("UPDATER_PRIVATE_KEY").is_err() {
                if let Err(e) = fs::metadata(&self.package.updater.private_key) {
                    return Err(SomeError(format!("Private key not found: {}", e)));
                }
            }
        }
        // Check if codesigning parameters are set (if enabled)
        if !self.prepare.codesign.skip_sign {
            // ToDo
        }
        // Check file/directory paths
        if check_paths {
            // Output folder cannot be checked as it may not exist yet
            match fs::canonicalize(&self.env.input_dir) {
                Ok(res) => self.env.input_dir = res,
                Err(e) => return Err(SomeError(format!("Input dir error: {}", e))),
            }
            match fs::canonicalize(&self.env.previous_dir) {
                Ok(res) => self.env.previous_dir = res,
                Err(e) => return Err(SomeError(format!("Previous dir error: {}", e))),
            }
            // Check other files (nsis script, updater, vcredist)
        }

        Ok(())
    }

    pub fn from_file(path: &Path) -> Config {
        let config: Option<Config> = fs::read_to_string(path)
            .ok()
            .and_then(|fc| toml::from_str(fc.as_str()).ok());

        if config.is_none() {
            panic!("Failed to parse config!")
        }

        config.unwrap()
    }
}
